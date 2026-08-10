//! @author 十四叔
//! @date 2026/07/31

//! 关闭按钮 —— 画两条对角线构成 × 符号，悬停时背景高亮。
//! 对标标题栏关闭按钮风格：文字不参与，纯矢量绘制。

use std::any::Any;

use danqing::event::Event;
use danqing::widget::{EventResult, MsgQueue, Node, Widget};
use danqing::{
    AnimationCtx, Color, Constraints, Key, NamedKey, Point, Rect, RectBatch, Size, TextBatch,
};

/// 颜色绑定闭包类型。
type ColorBinding = Box<dyn Fn(&dyn Any) -> Color>;
/// 消息工厂闭包类型。
type MsgFactory = Box<dyn Fn() -> Box<dyn Any>>;

/// 关闭按钮：固定尺寸的 × 符号，矢量绘制。
pub struct CloseButton {
    /// 按钮区域 (layout 后赋值)。
    area: Rect,
    /// 是否悬停。
    hovered: bool,
    /// 是否按下。
    pressed: bool,
    /// 点击时产出的消息工厂。
    on_click: Option<MsgFactory>,
    /// 符号颜色绑定：每帧从应用状态读取。
    color_binding: Option<ColorBinding>,
    /// 最近一帧同步的符号颜色。
    symbol_color: Color,
    /// 悬停背景色绑定。
    hover_binding: Option<ColorBinding>,
    /// 最近一帧同步的悬停背景色。
    hover_bg: Color,
}

impl CloseButton {
    /// 创建关闭按钮 (默认符号色白色，无回调)。
    pub fn new() -> Self {
        Self {
            area: Rect::default(),
            hovered: false,
            pressed: false,
            on_click: None,
            color_binding: None,
            symbol_color: Color::rgb(0.5, 0.5, 0.5),
            hover_binding: None,
            hover_bg: Color::TRANSPARENT,
        }
    }

    /// 设置点击回调。
    pub fn on_click<M: 'static>(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.on_click = Some(Box::new(move || Box::new(f()) as Box<dyn Any>) as MsgFactory);
        self
    }

    /// 绑定符号颜色：每帧从应用状态读取。
    pub fn bind_color<S: 'static>(mut self, f: impl Fn(&S) -> Color + 'static) -> Self {
        self.color_binding = Some(Box::new(move |state: &dyn Any| {
            f(state
                .downcast_ref::<S>()
                .expect("CloseButton 状态类型不匹配"))
        }) as ColorBinding);
        self
    }

    /// 绑定悬停背景色。
    pub fn bind_hover_color<S: 'static>(mut self, f: impl Fn(&S) -> Color + 'static) -> Self {
        self.hover_binding = Some(Box::new(move |state: &dyn Any| {
            f(state
                .downcast_ref::<S>()
                .expect("CloseButton 状态类型不匹配"))
        }) as ColorBinding);
        self
    }

    /// 绘制两条对角线 × (基于给定区域)。
    fn paint_x(area: Rect, rects: &mut RectBatch, color: Color) {
        let inset = 0.3;
        let thickness = area.size.width.min(area.size.height) * 0.085;

        let left = area.origin.x + area.size.width * inset;
        let right = area.origin.x + area.size.width * (1.0 - inset);
        let top = area.origin.y + area.size.height * inset;
        let bottom = area.origin.y + area.size.height * (1.0 - inset);

        push_diagonal(
            rects,
            Point::new(left, top),
            Point::new(right, bottom),
            thickness,
            color,
        );
        push_diagonal(
            rects,
            Point::new(right, top),
            Point::new(left, bottom),
            thickness,
            color,
        );
    }
}

impl Default for CloseButton {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for CloseButton {
    fn sync(&mut self, state: &dyn Any) {
        if let Some(bind) = &self.color_binding {
            self.symbol_color = bind(state);
        }
        if let Some(bind) = &self.hover_binding {
            self.hover_bg = bind(state);
        }
    }

    fn animate(&mut self, _ctx: &AnimationCtx) {}

    fn layout(&mut self, constraints: Constraints, _texts: &mut TextBatch) -> Size {
        // 固定 24×24 逻辑像素
        let size = constraints.constrain(Size::new(24.0, 24.0));
        self.area = Rect::new(Point::ZERO, size);
        size
    }

    fn paint(&self, area: Rect, rects: &mut RectBatch, _texts: &mut TextBatch) {
        let area = area.snap_to_pixels();
        if self.hovered {
            rects.push_rect(area, self.hover_bg, 4.0);
        }
        let scale = if self.pressed { 0.7 } else { 1.0 };
        let color = Color::rgba(
            self.symbol_color.r * scale,
            self.symbol_color.g * scale,
            self.symbol_color.b * scale,
            self.symbol_color.a,
        );
        Self::paint_x(area, rects, color);
    }

    fn event(&mut self, event: &Event, area: Rect, msgs: &mut MsgQueue) -> EventResult {
        self.area = area;
        match event {
            Event::CursorMoved(p) => {
                self.hovered = area.contains(*p);
                if self.hovered {
                    EventResult::Consumed
                } else {
                    self.pressed = false;
                    EventResult::Ignored
                }
            }
            Event::CursorLeft => {
                self.hovered = false;
                self.pressed = false;
                EventResult::Ignored
            }
            Event::MouseInput {
                pressed, position, ..
            } => {
                let inside = area.contains(*position);
                if *pressed {
                    if inside {
                        self.pressed = true;
                        EventResult::Consumed
                    } else {
                        EventResult::Ignored
                    }
                } else {
                    let clicked = self.pressed && inside;
                    self.pressed = false;
                    if clicked {
                        if let Some(factory) = &self.on_click {
                            msgs.push(factory());
                        }
                        EventResult::Consumed
                    } else {
                        EventResult::Ignored
                    }
                }
            }
            Event::Key {
                key: Key::Named(NamedKey::Enter | NamedKey::Space),
                pressed: true,
                ..
            } => {
                if let Some(factory) = &self.on_click {
                    msgs.push(factory());
                }
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn children(&self) -> &[Node] {
        &[]
    }
}

/// 用小圆点队列近似对角线 (复用 TitleBar 的算法)。
fn push_diagonal(rects: &mut RectBatch, p1: Point, p2: Point, thickness: f32, color: Color) {
    if thickness <= 0.0 {
        return;
    }
    let dx = p2.x - p1.x;
    let dy = p2.y - p1.y;
    let length = (dx * dx + dy * dy).sqrt();
    if length < 1e-6 {
        return;
    }
    let half = thickness * 0.5;
    let step = thickness * 0.5;
    let count = (length / step).ceil().max(1.0) as usize;
    for i in 0..count {
        let t = (i as f32 + 0.5) / count as f32;
        let cx = p1.x + dx * t;
        let cy = p1.y + dy * t;
        rects.push_rect(
            Rect::from_xywh(cx - half, cy - half, thickness, thickness),
            color,
            half,
        );
    }
}
