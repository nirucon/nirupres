//! The small part of ZIP a `.nirupres` package needs: reading stored and
//! deflated entries (streamed, CRC checked) and writing a new package.
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub struct Entry {
    pub name: String,
    pub method: u16,
    pub crc: u32,
    pub csize: u64,
    pub size: u64,
    pub offset: u64,
    pub external_attr: u32,
}

pub fn crc32_update(mut crc: u32, data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, v) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *v = c;
        }
        t
    });
    crc = !crc;
    for &b in data {
        crc = t[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    !crc
}

fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

pub struct Reader {
    file: File,
    pub entries: Vec<Entry>,
}

/// True when the file has a ZIP end-of-central-directory record.
pub fn is_zip(path: &Path) -> bool {
    File::open(path)
        .ok()
        .and_then(|mut f| find_eocd(&mut f).ok())
        .is_some()
}

fn find_eocd(f: &mut File) -> std::io::Result<(u64, Vec<u8>)> {
    let len = f.seek(SeekFrom::End(0))?;
    let back = len.min(65_535 + 22);
    f.seek(SeekFrom::Start(len - back))?;
    let mut tail = vec![0u8; back as usize];
    f.read_exact(&mut tail)?;
    if tail.len() >= 22 {
        for i in (0..=tail.len() - 22).rev() {
            if u32le(&tail, i) == 0x0605_4b50 {
                return Ok((len - back + i as u64, tail[i..].to_vec()));
            }
        }
    }
    Err(std::io::Error::other("not a zip file"))
}

impl Reader {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut file = File::open(path).map_err(|e| e.to_string())?;
        let (_, eocd) = find_eocd(&mut file).map_err(|e| e.to_string())?;
        let count = u16le(&eocd, 10) as usize;
        let cd_size = u32le(&eocd, 12) as u64;
        let cd_off = u32le(&eocd, 16) as u64;
        if cd_off == 0xFFFF_FFFF || count == 0xFFFF {
            return Err("ZIP64 packages are not supported.".into());
        }
        if cd_size > 64 * 1024 * 1024 {
            return Err("Project package directory is too large.".into());
        }
        file.seek(SeekFrom::Start(cd_off))
            .map_err(|e| e.to_string())?;
        let mut cd = vec![0u8; cd_size as usize];
        file.read_exact(&mut cd).map_err(|e| e.to_string())?;
        let mut entries = Vec::new();
        let mut p = 0usize;
        while entries.len() < count {
            if p + 46 > cd.len() || u32le(&cd, p) != 0x0201_4b50 {
                return Err("Damaged ZIP directory.".into());
            }
            let method = u16le(&cd, p + 10);
            let crc = u32le(&cd, p + 16);
            let csize = u32le(&cd, p + 20) as u64;
            let size = u32le(&cd, p + 24) as u64;
            let nlen = u16le(&cd, p + 28) as usize;
            let xlen = u16le(&cd, p + 30) as usize;
            let clen = u16le(&cd, p + 32) as usize;
            let external_attr = u32le(&cd, p + 38);
            let offset = u32le(&cd, p + 42) as u64;
            if p + 46 + nlen > cd.len() {
                return Err("Damaged ZIP directory.".into());
            }
            let flags = u16le(&cd, p + 8);
            let raw = &cd[p + 46..p + 46 + nlen];
            let name = if flags & 0x800 != 0 {
                String::from_utf8_lossy(raw).into_owned()
            } else {
                // CP437 for plain names; ASCII is the same either way.
                raw.iter().map(|&b| cp437(b)).collect()
            };
            entries.push(Entry {
                name,
                method,
                crc,
                csize,
                size,
                offset,
                external_attr,
            });
            p += 46 + nlen + xlen + clen;
        }
        Ok(Self { file, entries })
    }

    fn data_start(&mut self, offset: u64, name: &str) -> Result<u64, String> {
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;
        let mut h = [0u8; 30];
        self.file.read_exact(&mut h).map_err(|e| e.to_string())?;
        if u32le(&h, 0) != 0x0403_4b50 {
            return Err(format!("Damaged ZIP entry: {name}"));
        }
        Ok(offset + 30 + u16le(&h, 26) as u64 + u16le(&h, 28) as u64)
    }

    /// Stream one entry into `out`, checking size and CRC.
    pub fn extract(&mut self, idx: usize, out: &mut dyn Write) -> Result<(), String> {
        let (name, method, crc, csize, size, offset) = {
            let e = &self.entries[idx];
            (e.name.clone(), e.method, e.crc, e.csize, e.size, e.offset)
        };
        let start = self.data_start(offset, &name)?;
        self.file
            .seek(SeekFrom::Start(start))
            .map_err(|e| e.to_string())?;
        let mut left = csize;
        let mut got_crc = 0u32;
        let mut got = 0u64;
        let mut buf = vec![0u8; 64 * 1024];
        match method {
            0 => {
                while left > 0 {
                    let n = (left as usize).min(buf.len());
                    self.file
                        .read_exact(&mut buf[..n])
                        .map_err(|e| e.to_string())?;
                    got_crc = crc32_update(got_crc, &buf[..n]);
                    got += n as u64;
                    out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                    left -= n as u64;
                }
            }
            8 => {
                use miniz_oxide::inflate::stream::{inflate, InflateState};
                use miniz_oxide::{DataFormat, MZFlush, MZStatus};
                let mut st = InflateState::new_boxed(DataFormat::Raw);
                let mut outbuf = vec![0u8; 128 * 1024];
                let mut inbuf: Vec<u8> = Vec::new();
                let mut done = false;
                while !done {
                    if inbuf.is_empty() && left > 0 {
                        let n = (left as usize).min(buf.len());
                        self.file
                            .read_exact(&mut buf[..n])
                            .map_err(|e| e.to_string())?;
                        inbuf.extend_from_slice(&buf[..n]);
                        left -= n as u64;
                    }
                    let flush = if left == 0 {
                        MZFlush::Finish
                    } else {
                        MZFlush::None
                    };
                    let r = inflate(&mut st, &inbuf, &mut outbuf, flush);
                    inbuf.drain(..r.bytes_consumed);
                    if r.bytes_written > 0 {
                        let chunk = &outbuf[..r.bytes_written];
                        got_crc = crc32_update(got_crc, chunk);
                        got += chunk.len() as u64;
                        if got > size {
                            return Err(format!("Damaged ZIP entry: {name}"));
                        }
                        out.write_all(chunk).map_err(|e| e.to_string())?;
                    }
                    match r.status {
                        Ok(MZStatus::StreamEnd) => done = true,
                        Ok(_) => {
                            if r.bytes_consumed == 0
                                && r.bytes_written == 0
                                && inbuf.is_empty()
                                && left == 0
                            {
                                return Err(format!("Damaged ZIP entry: {name}"));
                            }
                        }
                        Err(_) => return Err(format!("Damaged ZIP entry: {name}")),
                    }
                }
            }
            m => return Err(format!("Unsupported compression ({m}) in {name}")),
        }
        if got != size || got_crc != crc {
            return Err(format!("Bad CRC or size in ZIP entry: {name}"));
        }
        Ok(())
    }

    pub fn read(&mut self, idx: usize) -> Result<Vec<u8>, String> {
        let mut v = Vec::with_capacity(self.entries[idx].size.min(64 << 20) as usize);
        self.extract(idx, &mut v)?;
        Ok(v)
    }
}

fn cp437(b: u8) -> char {
    if b < 0x80 {
        b as char
    } else {
        const HI: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{a0}";
        HI.chars().nth((b - 0x80) as usize).unwrap_or('?')
    }
}

/// Writes a package: entries are added in order, the directory at the end.
pub struct Writer {
    out: File,
    pos: u64,
    dir: Vec<u8>,
    count: u16,
}

fn dos_time() -> (u16, u16) {
    let t = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    let time = ((tm.tm_hour as u16) << 11) | ((tm.tm_min as u16) << 5) | (tm.tm_sec as u16 / 2);
    let year = (tm.tm_year + 1900 - 1980).max(0) as u16;
    let date = (year << 9) | (((tm.tm_mon + 1) as u16) << 5) | tm.tm_mday as u16;
    (time, date)
}

impl Writer {
    pub fn create(path: &Path) -> Result<Self, String> {
        Ok(Self {
            out: File::create(path).map_err(|e| e.to_string())?,
            pos: 0,
            dir: Vec::new(),
            count: 0,
        })
    }

    fn header(
        &mut self,
        name: &str,
        method: u16,
        crc: u32,
        csize: u64,
        size: u64,
    ) -> Result<(), String> {
        if csize > 0xFFFF_FFF0 || size > 0xFFFF_FFF0 {
            return Err(format!("{name} is too large for a package"));
        }
        let (time, date) = dos_time();
        let flags: u16 = if name.is_ascii() { 0 } else { 0x800 };
        let mut h = Vec::with_capacity(30 + name.len());
        h.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        h.extend_from_slice(&20u16.to_le_bytes());
        h.extend_from_slice(&flags.to_le_bytes());
        h.extend_from_slice(&method.to_le_bytes());
        h.extend_from_slice(&time.to_le_bytes());
        h.extend_from_slice(&date.to_le_bytes());
        h.extend_from_slice(&crc.to_le_bytes());
        h.extend_from_slice(&(csize as u32).to_le_bytes());
        h.extend_from_slice(&(size as u32).to_le_bytes());
        h.extend_from_slice(&(name.len() as u16).to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes());
        h.extend_from_slice(name.as_bytes());
        self.out.write_all(&h).map_err(|e| e.to_string())?;
        let d = &mut self.dir;
        d.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        d.extend_from_slice(&(0x0300u16 | 20).to_le_bytes()); // made by Unix
        d.extend_from_slice(&20u16.to_le_bytes());
        d.extend_from_slice(&flags.to_le_bytes());
        d.extend_from_slice(&method.to_le_bytes());
        d.extend_from_slice(&time.to_le_bytes());
        d.extend_from_slice(&date.to_le_bytes());
        d.extend_from_slice(&crc.to_le_bytes());
        d.extend_from_slice(&(csize as u32).to_le_bytes());
        d.extend_from_slice(&(size as u32).to_le_bytes());
        d.extend_from_slice(&(name.len() as u16).to_le_bytes());
        d.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
        d.extend_from_slice(&((0o100644u32) << 16).to_le_bytes());
        d.extend_from_slice(&(self.pos as u32).to_le_bytes());
        d.extend_from_slice(name.as_bytes());
        self.pos += h.len() as u64;
        self.count += 1;
        Ok(())
    }

    /// Add bytes compressed with deflate.
    pub fn add_deflated(&mut self, name: &str, data: &[u8]) -> Result<(), String> {
        let packed = miniz_oxide::deflate::compress_to_vec(data, 6);
        let crc = crc32_update(0, data);
        self.header(name, 8, crc, packed.len() as u64, data.len() as u64)?;
        self.out.write_all(&packed).map_err(|e| e.to_string())?;
        self.pos += packed.len() as u64;
        Ok(())
    }

    /// Add bytes as they are (images and video are compressed already).
    pub fn add_stored(&mut self, name: &str, data: &[u8]) -> Result<(), String> {
        let crc = crc32_update(0, data);
        self.header(name, 0, crc, data.len() as u64, data.len() as u64)?;
        self.out.write_all(data).map_err(|e| e.to_string())?;
        self.pos += data.len() as u64;
        Ok(())
    }

    /// Add a file stored, streamed (two passes: CRC, then copy).
    pub fn add_file_stored(&mut self, name: &str, path: &Path) -> Result<(), String> {
        let mut f = File::open(path).map_err(|e| e.to_string())?;
        let mut crc = 0u32;
        let mut size = 0u64;
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let n = f.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            crc = crc32_update(crc, &buf[..n]);
            size += n as u64;
        }
        self.header(name, 0, crc, size, size)?;
        f.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut left = size;
        while left > 0 {
            let n = f.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                return Err(format!("{name} changed while saving"));
            }
            let n = (n as u64).min(left) as usize;
            self.out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            left -= n as u64;
        }
        self.pos += size;
        Ok(())
    }

    pub fn finish(mut self) -> Result<(), String> {
        let dir_off = self.pos;
        self.out.write_all(&self.dir).map_err(|e| e.to_string())?;
        let mut e = Vec::new();
        e.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        e.extend_from_slice(&[0, 0, 0, 0]);
        e.extend_from_slice(&self.count.to_le_bytes());
        e.extend_from_slice(&self.count.to_le_bytes());
        e.extend_from_slice(&(self.dir.len() as u32).to_le_bytes());
        e.extend_from_slice(&(dir_off as u32).to_le_bytes());
        e.extend_from_slice(&0u16.to_le_bytes());
        self.out.write_all(&e).map_err(|e| e.to_string())?;
        self.out.sync_all().map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_of_known_text() {
        assert_eq!(crc32_update(0, b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn write_then_read_back() {
        let dir = std::env::temp_dir().join(format!("nirupres-zip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.zip");
        let mut w = Writer::create(&p).unwrap();
        let big: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        w.add_deflated("project.json", b"{\"a\": 1}").unwrap();
        w.add_stored("assets/x.bin", &big).unwrap();
        w.finish().unwrap();
        assert!(is_zip(&p));
        let mut r = Reader::open(&p).unwrap();
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.read(0).unwrap(), b"{\"a\": 1}");
        assert_eq!(r.read(1).unwrap(), big);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
