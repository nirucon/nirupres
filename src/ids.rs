//! Identifiers of editor controls and actions.

/// Every command (toolbar, menus, palette, shortcuts).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Act {
    New,
    Open,
    Save,
    SaveAs,
    ExportMenu,
    ExportPng,
    ExportAllPng,
    ExportPdf,
    ExportPptx,
    ExportMd,
    ExportAiBrief,
    CopyAiPrompt,
    ExportAiTemplate,
    ImportMd,
    AddSlide,
    Duplicate,
    Delete,
    Undo,
    Redo,
    Focus,
    ChromeToggle,
    ChromeFollow(bool),
    Palette,
    About,
    Present,
    PresentStart,
    PresenterView,
    TalkMenu,
    Talk(u8),
    TalkHelp,
    Online,
    OnlineStart,
    Overview,
    CopySlides,
    PasteSlides,
    SameLayout,
    MoveUp,
    MoveDown,
    DeckSettings,
    FileWorkflow,
    DeckHealth,
    AssetManager,
    Profile(u8),
    PresentationCheck,
    ApplyLogo(u8),
    ApplyAccent(u8),
    ToggleOutline,
    HideLeft,
    HideRight,
    ToggleLeft,
    ToggleRight,
    PresetsMenu,
    Preset(u8),
    ChooseImage,
    RemoveImage,
    ChooseBg,
    RemoveBg,
    ChoosePoster,
    ChooseVideo,
    ApplyStyle,
    PreviewTransition,
    Overflow,
    DocTitle,
}

pub const TALK_PROFILES: [(&str, &str); 3] = [
    ("Screen + microphone", "screen-mic"),
    ("Screen + camera", "screen-camera"),
    ("Camera + microphone", "camera-mic"),
];

pub const LOGO_MODES: [&str; 3] = ["AUTO", "ON", "OFF"];
pub const ACCENTS: [&str; 7] = ["AUTO", "BLUE", "GREEN", "YELLOW", "RED", "PINK", "PURPLE"];

/// Text fields (single and multi-line).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fid {
    Footer,
    Title,
    Body,
    Img,
    BgImg,
    Caption,
    ImageRequest,
    VideoSource,
    VideoPoster,
    Notes,
    /// Fields of the open dialog.
    D(u8),
}

impl Fid {
    pub fn multiline(self) -> bool {
        matches!(self, Fid::Body | Fid::Notes) || matches!(self, Fid::D(n) if n >= 100)
    }
}

/// Combo boxes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cid {
    Theme,
    Font,
    Ratio,
    Transition,
    Recent,
    Layout,
    ListStyle,
    ImgMode,
    Treatment,
    FocalX,
    FocalY,
    BgFocalX,
    BgFocalY,
    BgMotion,
    SlideTheme,
    Logo,
    Accent,
    SlideFont,
    FontScale,
    Align,
    Weight,
    SlideTransition,
    D(u8),
}

/// Check boxes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kid {
    Numbers,
    Guides,
    Logo,
    BgMono,
    Autoplay,
    Muted,
    OverrideTheme,
    OverrideFont,
    Hidden,
    D(u8),
}

/// Sliders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sid {
    BgDim,
    BgBlur,
    Brightness,
    Contrast,
    Overlay,
    Blur,
    D(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Act(Act),
    Combo(Cid),
    Check(Kid),
    Slider(Sid),
    Field(Fid),
    Slide(usize),
    SlideList,
    Section(usize),
    Inspector,
    Canvas,
    SplitLeft,
    SplitRight,
    /// A dialog button or row.
    D(u16),
    DList(usize),
    DListArea,
    Popup,
    Modal,
}
