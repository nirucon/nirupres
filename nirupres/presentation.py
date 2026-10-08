"""NIRUPRES 2.0.0 — extracted application subsystem.

Refactored from the verified 1.7.3 baseline. Existing project and Markdown
format versions remain unchanged.
"""
import json, sys, zipfile, tempfile, shutil, uuid, re, stat, os, platform, hashlib
from collections import OrderedDict
from pathlib import Path
from PySide6.QtCore import Qt, QTimer, QSize, Signal, QRectF, QElapsedTimer, QEvent, QPropertyAnimation, QEasingCurve, QParallelAnimationGroup, QStandardPaths, QUrl, QPointF, QVariantAnimation
from PySide6.QtGui import QAction, QKeySequence, QShortcut, QPixmap, QPainter, QColor, QFont, QFontMetrics, QFontDatabase, QPdfWriter, QPageSize, QPageLayout, QImage, QImageReader, QIcon, QTextDocument, QAbstractTextDocumentLayout, QPainterPath
from PySide6.QtWidgets import (QApplication,QMainWindow,QWidget,QVBoxLayout,QHBoxLayout,QListWidget,QListWidgetItem,QLineEdit,QTextEdit,QComboBox,QPushButton,QLabel,QFileDialog,QSplitter,QMessageBox,QToolBar,QCheckBox,QDialog,QGridLayout,QDialogButtonBox,QGraphicsScene,QGraphicsPixmapItem,QGraphicsBlurEffect,QSlider,QGroupBox,QFormLayout,QMenu,QInputDialog,QGraphicsOpacityEffect)

from .foundation import *
from .foundation import _safe_zip_name, _validate_project_data, _cached_pixmap, _image_dimensions, _optimize_asset, _paired_font, _parse_two_column_body, _plain_markup, _inline_html, _format_markdown_selection, _IMAGE_CACHE, _IMAGE_CACHE_BYTES
class SlideCanvas(QWidget):
    editRequested=Signal()
    def __init__(self,compact=False): super().__init__(); self.s={}; self.theme='B&W'; self.i=0; self.total=1; self.showNumbers=False; self.showLogo=True; self.footerText=''; self.footerAlign='LEFT'; self.footerSize='MEDIUM'; self.compact=compact; self.guides=False; self.deckFont='Noto Sans'; self.aspect='16:9'; self.setMinimumSize(320,180)
    def render(self,s,theme,i,total,show=True,deck_font='Noto Sans',aspect='16:9',show_logo=True,footer_text='',footer_align='LEFT',footer_size='MEDIUM'): self.s=s; self.theme=theme; self.i=i; self.total=total; self.showNumbers=show; self.showLogo=show_logo; self.footerText=footer_text; self.footerAlign=normalize_footer_align(footer_align); self.footerSize=normalize_footer_size(footer_size); self.deckFont=deck_font; self.aspect=aspect; self.update()
    def paintEvent(self,e):
        p=QPainter(self)
        try:
            r=QRectF(self.rect())
            ar={'16:9':16/9,'16:10':16/10,'4:3':4/3,'A4':297/210}.get(self.aspect,16/9)
            if r.width()/max(1,r.height())>ar: nw=r.height()*ar; r=QRectF((self.width()-nw)/2,0,nw,self.height())
            else: nh=r.width()/ar; r=QRectF(0,(self.height()-nh)/2,self.width(),nh)
            p.fillRect(self.rect(),QColor('#0B0B0B')); p.fillRect(r.adjusted(-1,-1,1,1),QColor('#2A2A2A'))
            try:
                render_slide(p,r,self.s,self.theme,self.i,self.total,self.showNumbers and not self.compact,self.deckFont,self.showLogo,self.footerText,self.footerAlign,self.footerSize)
            except Exception as exc:
                print(f'NIRUPRES slide render warning: {exc}', file=sys.stderr)
                p.fillRect(r,QColor('#050505')); p.setPen(QColor('#F4F2ED')); p.drawText(r,Qt.AlignCenter,'Unable to render slide')
            if self.guides and not self.compact:
                p.save()
                try:
                    p.setPen(QColor(255,255,255,55)); safe=r.adjusted(r.width()*.055,r.height()*.055,-r.width()*.055,-r.height()*.055); p.drawRect(safe); p.drawLine(int(r.center().x()),int(r.top()),int(r.center().x()),int(r.bottom())); p.drawLine(int(r.left()),int(r.center().y()),int(r.right()),int(r.center().y()))
                finally:
                    p.restore()
        finally:
            if p.isActive(): p.end()
    def mouseDoubleClickEvent(self,e):
        if not self.compact: self.editRequested.emit(); e.accept()
        else: super().mouseDoubleClickEvent(e)

class Thumb(QWidget):
    def __init__(self,s,theme,i,total,show_logo=True,footer_text='',footer_align='LEFT',footer_size='MEDIUM'):
        super().__init__(); l=QVBoxLayout(self); l.setContentsMargins(4,4,4,5); l.setSpacing(3); c=SlideCanvas(True); c.setFixedSize(176,99); c.render(s,theme,i,total,False,show_logo=show_logo,footer_text=footer_text,footer_align=footer_align,footer_size=footer_size); l.addWidget(c); flags=('  ◇' if s.get('notes') else '')+('  ⊘' if s.get('hidden') else '')+('  ●' if s.get('themeOverride','DECK')!='DECK' else ''); q=QLabel(f'{i+1:02d}  {(s.get("title") or s.get("layout"))[:20]}{flags}'); q.setObjectName('thumbtitle'); l.addWidget(q)

class DropLine(QLineEdit):
    fileDropped=Signal(str)
    def __init__(self): super().__init__(); self.setAcceptDrops(True); self.setPlaceholderText('Drop image here or choose…')
    def dragEnterEvent(self,e):
        if e.mimeData().hasUrls(): e.acceptProposedAction()
    def dropEvent(self,e):
        for u in e.mimeData().urls():
            p=u.toLocalFile()
            if Path(p).suffix.lower() in ('.png','.jpg','.jpeg','.webp','.bmp'): self.fileDropped.emit(p); e.acceptProposedAction(); return

class TransitionEngine:
    """Small, allocation-bounded audience transition engine.

    It animates one snapshot overlay and never touches the slide renderer/model.
    This keeps editor rendering, exports and Presenter View deterministic.
    """
    def __init__(self, view):
        self.view=view; self.overlay=None; self.group=None
    def stop(self):
        if self.group is not None:
            try:self.group.stop()
            except RuntimeError:pass
        if self.overlay is not None:
            try:self.overlay.deleteLater()
            except RuntimeError:pass
        self.group=None; self.overlay=None
    def animate(self, old_pm, mode):
        self.stop()
        if old_pm is None or old_pm.isNull() or mode=='NONE': return
        overlay=QLabel(self.view); overlay.setPixmap(old_pm); overlay.setScaledContents(True); overlay.setGeometry(self.view.rect()); overlay.show(); overlay.raise_()
        effect=QGraphicsOpacityEffect(overlay); overlay.setGraphicsEffect(effect); effect.setOpacity(1.0)
        duration={'FADE':360,'MORPH':560,'REVEAL':460}.get(mode,360)
        fade=QPropertyAnimation(effect,b'opacity',overlay); fade.setDuration(duration); fade.setStartValue(1.0); fade.setEndValue(0.0); fade.setEasingCurve(QEasingCurve.OutCubic)
        group=QParallelAnimationGroup(overlay); group.addAnimation(fade)
        r=self.view.rect()
        if mode=='MORPH':
            dx=max(7,int(r.width()*.014)); dy=max(4,int(r.height()*.014))
            geo=QPropertyAnimation(overlay,b'geometry',overlay); geo.setDuration(duration); geo.setStartValue(r); geo.setEndValue(r.adjusted(-dx,-dy,dx,dy)); geo.setEasingCurve(QEasingCurve.OutCubic); group.addAnimation(geo)
        elif mode=='REVEAL':
            dx=max(8,int(r.width()*.012))
            geo=QPropertyAnimation(overlay,b'geometry',overlay); geo.setDuration(duration); geo.setStartValue(r); geo.setEndValue(r.translated(-dx,0)); geo.setEasingCurve(QEasingCurve.OutCubic); group.addAnimation(geo)
        def done():
            overlay.deleteLater(); self.overlay=None; self.group=None
        group.finished.connect(done); self.overlay=overlay; self.group=group; group.start()
    def opening(self, callback=None):
        # The black cover is created synchronously, before the first compositor frame.
        # Slide 1 is already rendered underneath it, so deck FADE and Ken Burns never
        # compete with the show-level opening transition.
        self.stop(); r=self.view.rect()
        if r.isEmpty():
            if callback: callback()
            return
        overlay=QLabel(self.view); overlay.setStyleSheet('background:#000;border:0'); overlay.setGeometry(r); overlay.show(); overlay.raise_()
        effect=QGraphicsOpacityEffect(overlay); overlay.setGraphicsEffect(effect); effect.setOpacity(1.0)
        fade=QPropertyAnimation(effect,b'opacity',overlay); fade.setDuration(1450); fade.setStartValue(1.0); fade.setEndValue(0.0); fade.setEasingCurve(QEasingCurve.InOutCubic)
        group=QParallelAnimationGroup(overlay); group.addAnimation(fade)
        def done():
            overlay.deleteLater(); self.overlay=None; self.group=None
            if callback: callback()
        group.finished.connect(done); self.overlay=overlay; self.group=group; group.start()

    def closing(self, black_reached):
        # Fade only to black here. Presenter owns the five-second black hold and
        # session lifetime, keeping visual animation separate from control state.
        self.stop(); overlay=QLabel(self.view); overlay.setStyleSheet('background:#000;border:0'); overlay.setGeometry(self.view.rect()); overlay.show(); overlay.raise_()
        effect=QGraphicsOpacityEffect(overlay); overlay.setGraphicsEffect(effect); effect.setOpacity(0.0)
        fade=QPropertyAnimation(effect,b'opacity',overlay); fade.setDuration(1750); fade.setStartValue(0.0); fade.setEndValue(1.0); fade.setEasingCurve(QEasingCurve.InOutCubic)
        group=QParallelAnimationGroup(overlay); group.addAnimation(fade)
        completed={'done':False}
        def reached():
            if completed['done']: return
            completed['done']=True; black_reached()
        group.finished.connect(reached); self.overlay=overlay; self.group=group; group.start()
        QTimer.singleShot(1900,reached)

class PresenterFocusOverlay(QWidget):
    """Audience-only focus system: soft Spotlight, deep Focus Reveal and pinned Focus Trail."""
    def __init__(self,parent):
        super().__init__(parent); self.mode=None; self.point=QPointF(); self.radius=165; self.pinned=False; self._move_anim=None
        self.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents,True)
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground,True); self.hide()
    def set_mode(self,mode):
        if self.mode==mode: mode=None
        self.mode=mode; self.pinned=False
        if mode=='SPOTLIGHT': self.radius=165
        elif mode=='FOCUS': self.radius=115
        self.setVisible(bool(mode)); self.raise_(); self.update()
    def set_point(self,p,force=False):
        if self.pinned and not force:return
        self.point=QPointF(p); self.update()
    def pin_or_move(self,p):
        if not self.mode:return False
        target=QPointF(p)
        if not self.pinned:
            self.pinned=True; self.point=target; self.update(); return True
        start=QPointF(self.point); anim=QVariantAnimation(self); anim.setDuration(360); anim.setStartValue(start); anim.setEndValue(target); anim.setEasingCurve(QEasingCurve.InOutCubic)
        anim.valueChanged.connect(lambda v:self.set_point(v,True)); self._move_anim=anim; anim.start(); return True
    def unpin(self): self.pinned=False
    def adjust_radius(self,delta): self.radius=max(65,min(420,self.radius+delta)); self.update()
    def paintEvent(self,e):
        if not self.mode:return
        q=QPainter(self); q.setRenderHint(QPainter.RenderHint.Antialiasing,True)
        outer=QPainterPath(); outer.addRect(QRectF(self.rect()))
        hole=QPainterPath(); hole.addEllipse(self.point,self.radius,self.radius)
        shade=outer.subtracted(hole)
        # Spotlight keeps context readable. Focus Reveal deliberately isolates the subject.
        alpha=112 if self.mode=='SPOTLIGHT' else 218
        q.fillPath(shade,QColor(0,0,0,alpha))
        # A restrained feather ring prevents Focus Reveal from looking like a hard cut-out.
        if self.mode=='FOCUS':
            q.setPen(QColor(255,255,255,20)); q.drawEllipse(self.point,self.radius,self.radius)
        q.end()

def _web_video_url(url):
    url=str(url or '').strip()
    m=re.search(r'(?:youtube\.com/watch\?v=|youtu\.be/)([A-Za-z0-9_-]{6,})',url)
    if m:return f'https://www.youtube-nocookie.com/embed/{m.group(1)}?rel=0&modestbranding=1'
    m=re.search(r'vimeo\.com/(\d+)',url)
    if m:return f'https://player.vimeo.com/video/{m.group(1)}'
    return url

class PresenterPauseOverlay(QWidget):
    """Audience-only pause veil with an independent count-up clock."""
    def __init__(self,parent):
        super().__init__(parent); self._clock=QElapsedTimer(); self._seconds=0
        self.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents,True)
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground,True)
        self._tick=QTimer(self); self._tick.setTimerType(Qt.TimerType.PreciseTimer); self._tick.setInterval(100); self._tick.timeout.connect(self._update_time); self._opacity=QGraphicsOpacityEffect(self); self.setGraphicsEffect(self._opacity); self._opacity.setOpacity(0.0); self._fade=None; self.hide()
    def start_pause(self):
        self._seconds=0; self._clock.restart(); self._tick.start(); self.show(); self.raise_(); self._animate_opacity(1.0,380); self.update()
    def stop_pause(self):
        self._tick.stop(); self._animate_opacity(0.0,300,hide_after=True)
    def _animate_opacity(self,target,duration,hide_after=False):
        if self._fade is not None:
            try:self._fade.stop()
            except Exception:pass
        anim=QPropertyAnimation(self._opacity,b'opacity',self); anim.setDuration(duration); anim.setStartValue(self._opacity.opacity()); anim.setEndValue(target); anim.setEasingCurve(QEasingCurve.InOutCubic)
        if hide_after:anim.finished.connect(self.hide)
        self._fade=anim; anim.start()
    def elapsed_seconds(self):
        # QElapsedTimer is the source of truth. QTimer only schedules repaints;
        # delayed timer delivery must never make the displayed pause clock stale.
        if not self._clock.isValid(): return 0
        return max(0,self._clock.elapsed()//1000)
    def _update_time(self):
        self._seconds=self.elapsed_seconds()
        self.update()
    def paintEvent(self,event):
        q=QPainter(self); q.setRenderHint(QPainter.RenderHint.Antialiasing,True)
        # Deep translucent veil: the slide remains recognizable without competing for attention.
        q.fillRect(self.rect(),QColor(0,0,0,205))
        cx=self.rect().center().x(); cy=self.rect().center().y(); fg=QColor('#F4F2ED'); muted=QColor(244,242,237,165)
        title=QFont('Noto Sans'); title.setPixelSize(max(28,int(min(self.width(),self.height())*.055))); title.setWeight(QFont.Weight.DemiBold); title.setLetterSpacing(QFont.SpacingType.AbsoluteSpacing,4)
        q.setFont(title); q.setPen(fg); tr=QRectF(0,cy-title.pixelSize()*1.35,self.width(),title.pixelSize()*1.5); q.drawText(tr,Qt.AlignmentFlag.AlignHCenter|Qt.AlignmentFlag.AlignVCenter,'PAUS')
        sec=self.elapsed_seconds(); self._seconds=sec; clock=f'{sec//60:02d}:{sec%60:02d}'; tf=QFont('Noto Sans Mono'); tf.setPixelSize(max(20,int(min(self.width(),self.height())*.034))); tf.setWeight(QFont.Weight.Medium)
        q.setFont(tf); q.setPen(muted); rr=QRectF(0,cy+title.pixelSize()*.35,self.width(),tf.pixelSize()*1.8); q.drawText(rr,Qt.AlignmentFlag.AlignHCenter|Qt.AlignmentFlag.AlignVCenter,clock)
        q.end()

class Audience(QMainWindow):
    keyForward=Signal(int,int,str)
    closeRequested=Signal()
    def __init__(self,data,start=0,online=False):
        super().__init__(); self.data=data; self.i=start; self.online=online; self.view=SlideCanvas(); self.blank=None; self.transition_engine=TransitionEngine(self.view); self.setCentralWidget(self.view); self.setFocusPolicy(Qt.StrongFocus)
        self._video_widget=None; self._video_player=None; self._video_audio=None; self._video_source_key=''; self._motion_clock=QElapsedTimer(); self._motion_timer=QTimer(self); self._motion_timer.setInterval(40); self._motion_timer.timeout.connect(self.draw); self._motion_clock.start(); self._motion_timer.start(); self.setWindowTitle('NIRUPRES — Audience' + (' · SHARE THIS WINDOW' if online else ''))
        self._opening=False; self._curtain=None; self._zoom_overlay=None; self._zoomed=False; self._paused=False; self._pause_video_was_playing=False; self._pause_web_audio_muted=None; self.focus_overlay=PresenterFocusOverlay(self.view); self.pause_overlay=PresenterPauseOverlay(self.view); self.view.setMouseTracking(True); self.view.installEventFilter(self)
        self._cursor_timer=QTimer(self); self._cursor_timer.setSingleShot(True); self._cursor_timer.setInterval(1500); self._cursor_timer.timeout.connect(lambda:self.setCursor(Qt.BlankCursor))
        if not online: self._cursor_timer.start()
        self.draw()
    def draw(self):
        if self.blank is not None:return
        slide=self.data['slides'][self.i]; render_slide_data=slide
        if slide.get('backgroundImage') and slide.get('backgroundMotion')=='KEN BURNS' and not self.data.get('reduceMotion',False):
            motion_key=str(slide.get('id') or self.i)
            if getattr(self,'_motion_slide_key',None)!=motion_key:
                self._motion_slide_key=motion_key; self._motion_clock.restart()
            render_slide_data=clone(slide)
            phase=min(1.0,self._motion_clock.elapsed()/18000.0); ease=phase*phase*(3.0-2.0*phase)
            direction=-1.0 if (sum(ord(c) for c in motion_key)%2) else 1.0; drift=(ease-.5)*.12*direction
            render_slide_data['backgroundFocalX']=max(0.0,min(1.0,float(slide.get('backgroundFocalX',.5))+drift))
            render_slide_data['backgroundFocalY']=max(0.0,min(1.0,float(slide.get('backgroundFocalY',.5))-drift*.35))
            render_slide_data['_backgroundZoom']=1.0+.085*ease
        self.view.render(render_slide_data,self.data.get('theme','B&W'),self.i,len(self.data['slides']),self.data.get('showNumbers',False),self.data.get('fontFamily','Noto Sans'),self.data.get('aspect','16:9'),self.data.get('showLogo',True),self.data.get('footerText',''),self.data.get('footerAlign','LEFT'),self.data.get('footerSize','MEDIUM'))
        if not self._opening:self._sync_video(slide)
        self.focus_overlay.setGeometry(self.view.rect()); self.focus_overlay.raise_(); self.pause_overlay.setGeometry(self.view.rect());
        if self._paused:self.pause_overlay.raise_()
    def _clear_video(self):
        if self._video_player is not None:
            try:self._video_player.stop()
            except Exception:pass
        if self._video_widget is not None:
            try:self._video_widget.hide(); self._video_widget.deleteLater()
            except Exception:pass
        self._video_widget=None; self._video_player=None; self._video_audio=None; self._video_source_key=''
    def _video_rect(self,slide):
        r=QRectF(self.view.rect()); ar={'16:9':16/9,'16:10':16/10,'4:3':4/3,'A4':297/210}.get(self.data.get('aspect','16:9'),16/9)
        if r.width()/max(1,r.height())>ar: nw=r.height()*ar; r=QRectF((self.view.width()-nw)/2,0,nw,self.view.height())
        else: nh=r.width()/ar; r=QRectF(0,(self.view.height()-nh)/2,self.view.width(),nh)
        m=r.width()*.055
        vr=QRectF(r.x()+m,r.y()+r.height()*.23,r.width()-2*m,r.height()*.58) if slide.get('layout')=='VIDEO' else QRectF(r.x()+m,r.y()+r.height()*.25,r.width()*.56,r.height()*.52)
        return vr.toRect()
    def _sync_video(self,slide):
        source=str(slide.get('videoSource','') or '').strip(); is_video=slide.get('layout') in ('VIDEO','VIDEO + TEXT') and bool(source)
        if not is_video:
            if self._video_widget is not None:self._clear_video()
            return
        key=f'{self.i}|{source}'
        if key!=self._video_source_key:
            self._clear_video(); self._video_source_key=key
            try:
                if re.match(r'^https?://',source,re.I):
                    from PySide6.QtWebEngineWidgets import QWebEngineView
                    w=QWebEngineView(self.view); w.setUrl(QUrl(_web_video_url(source))); self._video_widget=w
                else:
                    from PySide6.QtMultimedia import QMediaPlayer, QAudioOutput
                    from PySide6.QtMultimediaWidgets import QVideoWidget
                    w=QVideoWidget(self.view); player=QMediaPlayer(self); audio=QAudioOutput(self); audio.setMuted(bool(slide.get('videoMuted',False))); player.setAudioOutput(audio); player.setVideoOutput(w); player.setSource(QUrl.fromLocalFile(source)); self._video_widget=w; self._video_player=player; self._video_audio=audio
                    if slide.get('videoAutoplay',False): player.play()
                self._video_widget.show(); self._video_widget.raise_()
            except Exception as exc:
                print(f'NIRUPRES video warning: {exc}',file=sys.stderr); self._clear_video(); return
        if self._video_widget is not None:
            self._video_widget.setGeometry(self._video_rect(slide)); self._video_widget.show(); self._video_widget.raise_()
    def toggle_video(self):
        if self._video_player is None:return False
        try:
            from PySide6.QtMultimedia import QMediaPlayer
            if self._video_player.playbackState()==QMediaPlayer.PlayingState:self._video_player.pause()
            else:self._video_player.play()
            return True
        except Exception:return False
    def _transition_for(self,i):
        slide=self.data['slides'][i]
        return 'NONE' if self.data.get('reduceMotion',False) else normalize_transition(slide.get('transition','DECK'),True).replace('DECK',normalize_transition(self.data.get('transition','NONE')))
    def set_index(self,i):
        new_i=max(0,min(len(self.data['slides'])-1,i))
        if new_i==self.i: return
        mode=self._transition_for(new_i)
        old_pm=self.view.grab() if self.blank is None and self.view.isVisible() and mode!='NONE' else None
        self.clear_presenter_effects(); self.i=new_i; self._motion_clock.restart(); self.draw()
        if old_pm is None or old_pm.isNull() or mode=='NONE': return
        self.transition_engine.animate(old_pm,mode)
    def set_blank(self,mode=None):
        self.blank=mode
        if mode=='B': self.setStyleSheet('background:#000'); self.view.hide()
        elif mode=='W': self.setStyleSheet('background:#fff'); self.view.hide()
        else: self.setStyleSheet(''); self.view.show(); self.draw()
    def begin_opening(self):
        self._opening=True; self._motion_clock.restart(); self.draw()
        self.transition_engine.opening(self.finish_opening)
    def finish_opening(self):
        self._opening=False; self.draw()
    def set_focus_mode(self,mode):
        self.focus_overlay.set_mode(None if self.focus_overlay.mode==mode else mode)
        if self.focus_overlay.mode and self.focus_overlay.point.isNull(): self.focus_overlay.set_point(self.view.rect().center())
        self._cursor_activity()
    def adjust_focus_radius(self,delta): self.focus_overlay.adjust_radius(delta)
    def toggle_zoom(self):
        # Live Zoom transforms a snapshot only; deck data and exports remain untouched.
        if self._zoomed:
            if self._zoom_overlay is not None:
                ov=self._zoom_overlay; anim=QPropertyAnimation(ov,b'geometry',ov); anim.setDuration(280); anim.setStartValue(ov.geometry()); anim.setEndValue(self.view.rect()); anim.setEasingCurve(QEasingCurve.InOutCubic)
                anim.finished.connect(lambda:(ov.deleteLater(),setattr(self,'_zoom_overlay',None))); self._zoom_anim=anim; anim.start()
            self._zoomed=False; return
        pm=self.view.grab();
        if pm.isNull(): return
        ov=QLabel(self.view); ov.setPixmap(pm); ov.setScaledContents(True); ov.setGeometry(self.view.rect()); ov.show(); ov.raise_()
        pt=self.focus_overlay.point if not self.focus_overlay.point.isNull() else QPointF(self.view.rect().center())
        r=self.view.rect(); scale=1.75; nw=int(r.width()*scale); nh=int(r.height()*scale); tx=int(r.center().x()-(pt.x()/max(1,r.width()))*nw); ty=int(r.center().y()-(pt.y()/max(1,r.height()))*nh); target=QRectF(tx,ty,nw,nh).toRect()
        anim=QPropertyAnimation(ov,b'geometry',ov); anim.setDuration(420); anim.setStartValue(r); anim.setEndValue(target); anim.setEasingCurve(QEasingCurve.InOutCubic); self._zoom_overlay=ov; self._zoom_anim=anim; self._zoomed=True; anim.start()
        self.focus_overlay.raise_()
    def clear_presenter_effects(self):
        if self._zoom_overlay is not None:
            self._zoom_overlay.deleteLater(); self._zoom_overlay=None
        self._zoomed=False; self.focus_overlay.unpin()
    def _cursor_activity(self):
        if self.online:return
        self.setCursor(Qt.ArrowCursor); self._cursor_timer.start()
    def set_curtain(self,on):
        if on:
            if self._curtain is None:
                self._curtain=QLabel(self.view); self._curtain.setStyleSheet('background:#000;border:0')
            self._curtain.setGeometry(self.view.rect()); self._curtain.show(); self._curtain.raise_(); self._motion_timer.stop()
        else:
            if self._curtain is not None:self._curtain.hide()
            if not self._motion_timer.isActive():self._motion_timer.start()
        self.focus_overlay.raise_()
    def set_pause(self,on):
        on=bool(on)
        if on==self._paused:return
        self._paused=on
        if on:
            self._motion_timer.stop()
            # Local video is genuinely paused and resumes only if it was playing.
            self._pause_video_was_playing=False
            if self._video_player is not None:
                try:
                    from PySide6.QtMultimedia import QMediaPlayer
                    self._pause_video_was_playing=self._video_player.playbackState()==QMediaPlayer.PlayingState
                    if self._pause_video_was_playing:self._video_player.pause()
                except Exception:pass
            # Web embeds cannot be cross-origin paused reliably; silence them while the pause veil is active.
            if self._video_widget is not None and hasattr(self._video_widget,'page'):
                try:
                    page=self._video_widget.page(); self._pause_web_audio_muted=page.isAudioMuted(); page.setAudioMuted(True)
                except Exception:self._pause_web_audio_muted=None
            self.pause_overlay.setGeometry(self.view.rect()); self.pause_overlay.start_pause()
        else:
            self.pause_overlay.stop_pause()
            if self._video_player is not None and self._pause_video_was_playing:
                try:self._video_player.play()
                except Exception:pass
            if self._video_widget is not None and hasattr(self._video_widget,'page') and self._pause_web_audio_muted is not None:
                try:self._video_widget.page().setAudioMuted(bool(self._pause_web_audio_muted))
                except Exception:pass
            self._pause_video_was_playing=False; self._pause_web_audio_muted=None
            if not (self._curtain is not None and self._curtain.isVisible()) and not self._motion_timer.isActive():self._motion_timer.start()
            self.focus_overlay.raise_()

    def eventFilter(self,obj,event):
        if obj is self.view and event.type()==QEvent.Type.MouseMove:
            self.focus_overlay.set_point(event.position()); self._cursor_activity()
        if obj is self.view and event.type()==QEvent.Type.MouseButtonPress and self.focus_overlay.mode:
            self.focus_overlay.pin_or_move(event.position()); self._cursor_activity(); return True
        return super().eventFilter(obj,event)
    def resizeEvent(self,e):
        super().resizeEvent(e); self.focus_overlay.setGeometry(self.view.rect())
        if self._curtain is not None:self._curtain.setGeometry(self.view.rect())
        self.pause_overlay.setGeometry(self.view.rect())
        if self._paused:self.pause_overlay.raise_()
    def keyPressEvent(self,e): self.keyForward.emit(int(e.key()),int(e.modifiers()),e.text()); e.accept()
    def mousePressEvent(self,e):
        self.keyForward.emit(int(Qt.Key_Left if e.position().x()<self.width()/2 else Qt.Key_Right),0,''); e.accept()
    def closeEvent(self,e):
        self._clear_video()
        if self.isVisible(): self.closeRequested.emit()
        e.accept()

class DisplaySetup(QDialog):
    def __init__(self,parent=None):
        super().__init__(parent); self.setWindowTitle('NIRUPRES — Display Setup'); self.setMinimumWidth(560); screens=QApplication.screens(); self.screens=screens
        v=QVBoxLayout(self); v.addWidget(QLabel('DISPLAY SETUP'))
        info=QLabel('Choose where Presenter View and the audience presentation should appear.'); info.setWordWrap(True); v.addWidget(info)
        f=QFormLayout(); self.presenter=QComboBox(); self.audience=QComboBox()
        for i,sc in enumerate(screens):
            g=sc.geometry(); label=f'{i+1}. {sc.name()} — {g.width()}×{g.height()} @ {g.x()},{g.y()}'+(' — PRIMARY' if sc==QApplication.primaryScreen() else '')
            self.presenter.addItem(label,i); self.audience.addItem(label,i)
        prefs={}
        try:prefs=json.loads(DISPLAY_PREFS.read_text(encoding='utf-8')) if DISPLAY_PREFS.exists() else {}
        except:pass
        def find(name,default):
            for i,sc in enumerate(screens):
                if sc.name()==name:return i
            return default
        pi=find(prefs.get('presenter',''),screens.index(QApplication.primaryScreen()) if QApplication.primaryScreen() in screens else 0)
        ai=find(prefs.get('audience',''),next((i for i in range(len(screens)) if i!=pi),pi))
        self.presenter.setCurrentIndex(pi); self.audience.setCurrentIndex(ai); f.addRow('Presenter display',self.presenter); f.addRow('Audience display',self.audience); v.addLayout(f)
        self.warn=QLabel(); self.warn.setWordWrap(True); v.addWidget(self.warn); self.presenter.currentIndexChanged.connect(self.check); self.audience.currentIndexChanged.connect(self.check); self.check()
        swap=QPushButton('⇄  SWAP DISPLAYS'); swap.setToolTip('Swap Presenter and Audience displays'); swap.clicked.connect(self.swap_displays); v.addWidget(swap)
        bb=neutralize_button_box(QDialogButtonBox(QDialogButtonBox.Ok|QDialogButtonBox.Cancel)); bb.accepted.connect(self.accept); bb.rejected.connect(self.reject); v.addWidget(bb)
    def check(self):
        same=self.presenter.currentData()==self.audience.currentData(); self.warn.setText('Presenter and Audience are set to the same display. Presenter View will not be shown.' if same else 'Only the slide is shown on Audience. Controls, notes, timer and next-slide preview stay on Presenter.')
    def swap_displays(self):
        pi=self.presenter.currentIndex(); ai=self.audience.currentIndex(); self.presenter.setCurrentIndex(ai); self.audience.setCurrentIndex(pi); self.check()
    def selection(self):
        pi=int(self.presenter.currentData()); ai=int(self.audience.currentData()); return self.screens[pi],self.screens[ai]

class Presenter(QMainWindow):
    finished=Signal()
    def __init__(self,data,start=0,presenter_screen=None,audience_screen=None,online=False):
        super().__init__(); self.data=data; self.online=online; self._closing=False; self._black_hold=False; self.curtain=False; self.paused=False; self._paused_total_ms=0; self._pause_started_ms=None; self.jump=''; self.blank=None; self.clock=QElapsedTimer(); self.clock.start(); self.setFocusPolicy(Qt.StrongFocus)
        screens=QApplication.screens(); primary=QApplication.primaryScreen(); presenter_screen=presenter_screen or primary; audience_screen=audience_screen or next((x for x in screens if x!=presenter_screen),presenter_screen)
        self.presenter_screen=presenter_screen; self.audience_screen=audience_screen; self.i=self._nearest_visible(start,1)
        QApplication.instance().screenRemoved.connect(self._screen_removed)
        self.audience=Audience(data,self.i,online=online); self.audience.keyForward.connect(self.handle_key); self.audience.closeRequested.connect(self.close_session)
        # Create native windows while still hidden, bind each QWindow to its target QScreen,
        # then enter fullscreen. Mapping first and moving later is unreliable under Wayland/Hyprland.
        self.audience.create()
        if self.audience.windowHandle(): self.audience.windowHandle().setScreen(audience_screen)
        if online:
            # Online mode deliberately uses a normal top-level Audience window. Teams,
            # Meet, Zoom and PipeWire portals can share this window without exposing notes.
            ag=audience_screen.availableGeometry(); aw=max(640,min(1100,int(ag.width()*0.58))); ah=max(360,min(680,int(aw*9/16)))
            ax=ag.x()+ag.width()-aw-24; ay=ag.y()+24
            self.audience.setGeometry(ax,ay,aw,ah); self.audience.setMinimumSize(640,360); self.audience.show()
        else:
            self.audience.setGeometry(audience_screen.geometry()); self.audience.showFullScreen()
        self.dual=online or (len(screens)>1 and presenter_screen!=audience_screen)
        if self.data.get('cinematicOpen',True) and not self.data.get('reduceMotion',False): self.audience.begin_opening()
        if not self.dual:
            # Single-display mode has no visible Presenter window. The Audience must
            # therefore be the explicit keyboard owner after Wayland has mapped the
            # fullscreen surface. Installing shortcuts on a hidden Presenter caused
            # navigation to be lost on Hyprland/Qt 6.11.
            self.hide(); QApplication.instance().installEventFilter(self); self._install_shortcuts(self.audience)
            QTimer.singleShot(0,self.claim_audience_focus); QTimer.singleShot(180,self.claim_audience_focus)
            return
        self.setWindowTitle('NIRUPRES — Presenter View'); self.resize(1180,760)
        root=QVBoxLayout(); host=QWidget(); host.setLayout(root); self.setCentralWidget(host)
        top=QHBoxLayout(); self.position_label=QLabel(); self.timer=QLabel('00:00'); self.audience_state=QLabel('SHARE: NIRUPRES — AUDIENCE' if online else f'AUDIENCE → {audience_screen.name()}'); self.audience_state.setObjectName('presenterState'); top.addWidget(QLabel('PRESENTER · ONLINE' if online else 'PRESENTER')); top.addStretch(); top.addWidget(self.audience_state); top.addSpacing(20); top.addWidget(self.position_label); top.addSpacing(20); top.addWidget(self.timer); root.addLayout(top)
        previews=QHBoxLayout(); previews.setSpacing(12)
        current_col=QVBoxLayout(); current_label=QLabel('CURRENT'); current_label.setObjectName('presenterLabel'); current_col.addWidget(current_label); self.now=SlideCanvas(); current_col.addWidget(self.now,1)
        next_col=QVBoxLayout(); next_label=QLabel('NEXT'); next_label.setObjectName('presenterLabel'); next_col.addWidget(next_label); self.next=SlideCanvas(); next_col.addWidget(self.next,1)
        previews.addLayout(current_col,3); previews.addLayout(next_col,1); root.addLayout(previews,3)
        controls=QHBoxLayout(); prev=QPushButton('‹  PREVIOUS'); end=QPushButton('✕  END PRESENTATION'); nxt=QPushButton('NEXT  ›')
        prev.clicked.connect(lambda:self.navigate(-1)); nxt.clicked.connect(lambda:self.navigate(1)); end.clicked.connect(self.close_session)
        end.setObjectName('endPresentation'); end.setToolTip('End presentation and return to the editor · Esc')
        controls.addWidget(prev); controls.addStretch(); controls.addWidget(end); controls.addStretch(); controls.addWidget(nxt); root.addLayout(controls)
        self.notes_label=QLabel('SPEAKER NOTES'); root.addWidget(self.notes_label); self.notes=QTextEdit(); self.notes.setReadOnly(True); self.notes.setMaximumHeight(190); root.addWidget(self.notes)
        self.tick=QTimer(self); self.tick.timeout.connect(self.update_clock); self.tick.start(1000)
        self.create()
        if self.windowHandle(): self.windowHandle().setScreen(presenter_screen)
        if online:
            pg=presenter_screen.availableGeometry(); pw=max(700,min(940,int(pg.width()*0.47))); ph=max(620,min(820,pg.height()-48))
            self.setGeometry(pg.x()+24,pg.y()+24,pw,ph); self.show()
        else:
            self.setGeometry(presenter_screen.geometry()); self.showFullScreen()
        self.draw()
        QApplication.instance().installEventFilter(self)
        # Presenter View/Online has two top-level windows. A single application-level
        # shortcut set prevents duplicate activations while allowing keyboard control
        # from either NIRUPRES presentation window. Audience.keyForward and the event
        # filter remain independent fallbacks for compositor/window-manager quirks.
        self._install_shortcuts(self)
        QTimer.singleShot(180,self.claim_focus)
    def _install_shortcuts(self,host):
        # Keep exactly one shortcut set per presentation session. ApplicationShortcut
        # makes navigation independent of which presentation child currently has focus.
        self._shortcuts=[]
        bindings=[
            ('Escape', lambda:self.close_session()),
            ('Right', lambda:None if self.paused else self.navigate(1)), ('Down', lambda:None if self.paused else self.navigate(1)),
            ('Space', lambda:None if self.paused else (self.navigate(1) if not self.audience.toggle_video() else None)), ('PgDown', lambda:None if self.paused else self.navigate(1)),
            ('Left', lambda:None if self.paused else self.navigate(-1)), ('Up', lambda:None if self.paused else self.navigate(-1)),
            ('PgUp', lambda:None if self.paused else self.navigate(-1)), ('Home', lambda:None if self.paused else self.navigate(-999)),
            ('End', lambda:None if self.paused else self.navigate(999)),
            ('B', lambda:None if self.paused else self.set_blank(None if self.blank=='B' else 'B')),
            ('W', lambda:None if self.paused else self.set_blank(None if self.blank=='W' else 'W')),
            ('S', lambda:None if self.paused else self.audience.set_focus_mode('SPOTLIGHT')),
            ('F', lambda:None if self.paused else self.audience.set_focus_mode('FOCUS')),
            ('Z', lambda:None if self.paused else self.audience.toggle_zoom()),
            ('C', lambda:None if self.paused else self.toggle_curtain()),
            ('P', lambda:self.toggle_pause()),
            ('[', lambda:None if self.paused else self.audience.adjust_focus_radius(-24)), (']', lambda:None if self.paused else self.audience.adjust_focus_radius(24)),
        ]
        for sequence,callback in bindings:
            sc=QShortcut(QKeySequence(sequence),host)
            sc.setContext(Qt.ShortcutContext.ApplicationShortcut)
            sc.activated.connect(callback); self._shortcuts.append(sc)
        for n in range(1,10):
            sc=QShortcut(QKeySequence(str(n)),host); sc.setContext(Qt.ShortcutContext.ApplicationShortcut)
            sc.activated.connect(lambda n=n:None if self.paused else self.navigate(10000+n-1)); self._shortcuts.append(sc)
    def _screen_removed(self,screen):
        if screen in (self.presenter_screen,self.audience_screen): self.close_session()
    def claim_focus(self):
        if not self.isVisible(): return
        self.raise_(); self.activateWindow(); self.setFocus(Qt.OtherFocusReason)
        if self.centralWidget(): self.centralWidget().setFocus(Qt.OtherFocusReason)
    def claim_audience_focus(self):
        if not self.audience or not self.audience.isVisible(): return
        self.audience.raise_(); self.audience.activateWindow(); self.audience.setFocus(Qt.OtherFocusReason)
        if self.audience.centralWidget(): self.audience.centralWidget().setFocus(Qt.OtherFocusReason)
    def _visible(self,i): return 0<=i<len(self.data['slides']) and not self.data['slides'][i].get('hidden',False)
    def _nearest_visible(self,i,d=1):
        n=len(self.data['slides']); i=max(0,min(n-1,i))
        if self._visible(i):return i
        for step in range(1,n):
            j=i+step*d
            if 0<=j<n and self._visible(j):return j
        for j in range(n):
            if self._visible(j):return j
        return i
    def _next_visible(self,i,d):
        j=i+d
        while 0<=j<len(self.data['slides']):
            if self._visible(j):return j
            j+=d
        return i
    def _active_elapsed_ms(self):
        current_pause=(self.clock.elapsed()-self._pause_started_ms) if self.paused and self._pause_started_ms is not None else 0
        return max(0,self.clock.elapsed()-self._paused_total_ms-current_pause)
    def update_clock(self):
        sec=self._active_elapsed_ms()//1000; label=f'{sec//60:02d}:{sec%60:02d}'
        target=int(self.data.get('targetMinutes',0) or 0)
        if target>0:
            visible=[j for j,x in enumerate(self.data['slides']) if not x.get('hidden')]
            pos=(visible.index(self.i)+1) if self.i in visible else 1; expected=(target*60)*(pos/max(1,len(visible))); delta=int(sec-expected); sign='+' if delta>=0 else '−'; d=abs(delta)
            label+=f'  ·  PACE {sign}{d//60:02d}:{d%60:02d}'
        self.timer.setText(label)
    def draw(self):
        total=len(self.data['slides']); s=self.data['slides'][self.i]
        # position_label exists only in Presenter View/Online. In single-display
        # mode QWidget.pos is a built-in method, so using self.pos as a label
        # caused every keyboard navigation redraw to raise AttributeError.
        if self.dual:
            self.position_label.setText(f'{self.i+1:02d} / {total:02d}')
            self.now.render(s,self.data.get('theme','B&W'),self.i,total,self.data.get('showNumbers',False),self.data.get('fontFamily','Noto Sans'),self.data.get('aspect','16:9'),self.data.get('showLogo',True),self.data.get('footerText',''),self.data.get('footerAlign','LEFT'),self.data.get('footerSize','MEDIUM'))
            notes=s.get('notes','').strip(); self.notes.setPlainText(notes); self.notes.setVisible(bool(notes)); self.notes_label.setVisible(bool(notes))
            ni=self._next_visible(self.i,1); self.next.render(self.data['slides'][ni],self.data.get('theme','B&W'),ni,total,False,self.data.get('fontFamily','Noto Sans'),self.data.get('aspect','16:9'),self.data.get('showLogo',True),self.data.get('footerText',''),self.data.get('footerAlign','LEFT'),self.data.get('footerSize','MEDIUM'))
        self.audience.set_index(self.i)
    def navigate(self,d):
        if d==-999:self.i=self._nearest_visible(0,1)
        elif d==999:self.i=self._nearest_visible(len(self.data['slides'])-1,-1)
        elif d>=10000:
            target=max(0,min(len(self.data['slides'])-1,d-10000)); self.i=self._nearest_visible(target,1)
        else:self.i=self._next_visible(self.i,1 if d>0 else -1)
        if self.blank is not None:self.set_blank(None)
        self.draw()
    def set_blank(self,mode):
        self.blank=mode; self.audience.set_blank(mode)
        if self.dual:self.audience_state.setText(('AUDIENCE: BLACK' if mode=='B' else 'AUDIENCE: WHITE' if mode=='W' else ('SHARE: NIRUPRES — AUDIENCE' if self.online else f'AUDIENCE → {self.audience_screen.name()}')))
    def toggle_curtain(self):
        self.curtain=not self.curtain; self.audience.set_curtain(self.curtain)
        if self.dual:self.audience_state.setText('CURTAIN — AUDIENCE SCREEN HIDDEN' if self.curtain else ('SHARE: NIRUPRES — AUDIENCE' if self.online else f'AUDIENCE → {self.audience_screen.name()}'))
    def toggle_pause(self):
        if self._closing:return
        if not self.paused:
            # Pause owns the audience state. Clear temporary blackout/curtain so PAUS is unambiguous.
            if self.blank is not None:self.set_blank(None)
            if self.curtain:self.toggle_curtain()
            self.paused=True; self._pause_started_ms=self.clock.elapsed(); self.audience.set_pause(True)
            if self.dual:self.audience_state.setText('PAUSED — AUDIENCE DIMMED')
        else:
            now=self.clock.elapsed()
            if self._pause_started_ms is not None:self._paused_total_ms+=max(0,now-self._pause_started_ms)
            self._pause_started_ms=None; self.paused=False; self.audience.set_pause(False)
            if self.dual:self.audience_state.setText('SHARE: NIRUPRES — AUDIENCE' if self.online else f'AUDIENCE → {self.audience_screen.name()}')
        self.update_clock() if self.dual else None
    def handle_key(self,key,mods=0,text=''):
        if key==Qt.Key_Escape:self.close_session();return
        if key==Qt.Key_P:self.toggle_pause();return
        if self.paused:return
        if key==Qt.Key_B:self.set_blank(None if self.blank=='B' else 'B');return
        if key==Qt.Key_W:self.set_blank(None if self.blank=='W' else 'W');return
        if key==Qt.Key_S:self.audience.set_focus_mode('SPOTLIGHT');return
        if key==Qt.Key_F:self.audience.set_focus_mode('FOCUS');return
        if key==Qt.Key_Z:self.audience.toggle_zoom();return
        if key==Qt.Key_C:self.toggle_curtain();return
        if key==Qt.Key_BracketLeft:self.audience.adjust_focus_radius(-24);return
        if key==Qt.Key_BracketRight:self.audience.adjust_focus_radius(24);return
        if self.blank is not None:self.set_blank(None)
        if Qt.Key_0<=key<=Qt.Key_9:self.jump+=text or str(key-int(Qt.Key_0));return
        if key in (Qt.Key_Return,Qt.Key_Enter) and self.jump:
            try:self.navigate(10000+int(self.jump)-1)
            finally:self.jump=''
            return
        if key==Qt.Key_Space and self.audience.toggle_video():return
        if key in (Qt.Key_Right,Qt.Key_Space,Qt.Key_PageDown,Qt.Key_Down):self.navigate(1)
        elif key in (Qt.Key_Left,Qt.Key_PageUp,Qt.Key_Up):self.navigate(-1)
        elif key==Qt.Key_Home:self.navigate(-999)
        elif key==Qt.Key_End:self.navigate(999)
    def eventFilter(self,watched,event):
        # Capture presentation keys even when a child widget/button owns focus.
        if event.type()==QEvent.Type.KeyPress:
            belongs=(watched is self or watched is self.audience)
            if isinstance(watched,QWidget):
                belongs=belongs or self.isAncestorOf(watched) or self.audience.isAncestorOf(watched)
            if belongs:
                key=int(event.key())
                presentation_keys=(Qt.Key_Escape,Qt.Key_B,Qt.Key_W,Qt.Key_S,Qt.Key_F,Qt.Key_Z,Qt.Key_C,Qt.Key_P,Qt.Key_BracketLeft,Qt.Key_BracketRight,Qt.Key_Return,Qt.Key_Enter,Qt.Key_Right,Qt.Key_Space,Qt.Key_PageDown,Qt.Key_Down,Qt.Key_Left,Qt.Key_PageUp,Qt.Key_Up,Qt.Key_Home,Qt.Key_End)
                if key in presentation_keys or Qt.Key_0<=key<=Qt.Key_9:
                    self.handle_key(key,int(event.modifiers()),event.text()); return True
        return super().eventFilter(watched,event)
    def keyPressEvent(self,e): self.handle_key(int(e.key()),int(e.modifiers()),e.text()); e.accept()
    def close_session(self):
        # A second Escape always means "close now", including during fade/black hold.
        if self._black_hold or getattr(self,'_closing_fade_started',False):
            self._finish_session(); return
        if self._closing:return
        if self.data.get('cinematicClose',True) and not self.data.get('reduceMotion',False):
            self._closing_fade_started=True; self.audience.transition_engine.closing(self._begin_black_hold); return
        self._finish_session()
    def _begin_black_hold(self):
        if self._closing:return
        self._black_hold=True
        # Keep audience black for five seconds; presenter may press Esc again to skip.
        QTimer.singleShot(5000,self._finish_session)
    def _finish_session(self):
        if self._closing:return
        self._closing=True; self._black_hold=False
        if self.paused:self.paused=False; self.audience.set_pause(False)
        if self.audience:
            self.audience.blockSignals(True); self.audience.close()
        self.close()
    def closeEvent(self,e):
        if not self._closing:
            self._closing=True
            if self.audience:self.audience.blockSignals(True);self.audience.close()
        try: QApplication.instance().removeEventFilter(self)
        except Exception: pass
        self.finished.emit(); e.accept()

