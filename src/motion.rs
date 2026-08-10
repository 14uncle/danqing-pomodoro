//! @author 十四叔
//! @date 2026/07/28

//! 场景动效策略 (纯逻辑): 哪个场景下雨/烧火/涌动、暂停沉降包络、强度权重合成。
//!
//! 与环境音同一美学契约 (潮汐式): 计时运行时世界环绕，暂停/空闲时
//! 世界退远 —— 火/海效包络以 `timer.is_running()` 为目标，500ms 滑动
//! (2026-07-28 spec 裁定：视觉沉降独立时长，不复用音频 300ms)。
//! 雨例外 (2026-07-29 用户裁定): 雨丝暂停时定格可见，强度不含包络;
//! 包络只推进雨钟 (main.rs `rain_clock`, 暂停 500ms 减速冻结 / 恢复加速续走)。
//! 时间由外部注入 (`AnimationCtx.elapsed` 累计值), 不读 wall-clock,
//! 可完整单元测试。
//!
//! 雨、火、海是并存标量而非互斥选择子：交叉淡化期间两端可同时非零
//! (spec: docs/specs/pomodoro-scene-motion-bonfire.md,
//!  docs/specs/pomodoro-scene-motion-sea.md)。

use std::time::Duration;

/// 雨场景在 `SCENES` 中的索引 (单测锁定名称，防生成器重排静默错位)。
pub const RAIN_SCENE: usize = 2;

/// 篝火场景在 `SCENES` 中的索引 (单测锁定名称，防生成器重排静默错位)。
pub const BONFIRE_SCENE: usize = 0;

/// 海场景在 `SCENES` 中的索引 (单测锁定名称，防生成器重排静默错位)。
pub const SEA_SCENE: usize = 1;

/// 山场景在 `SCENES` 中的索引 (单测锁定名称，防生成器重排静默错位)。
pub const MOUNTAIN_SCENE: usize = 3;

/// 森林场景在 `SCENES` 中的索引 (单测锁定名称，防生成器重排静默错位)。
pub const FOREST_SCENE: usize = 4;

/// 铁匠铺场景在 `SCENES` 中的索引。
pub const BLACKSMITH_SCENE: usize = 5;

/// 洞穴场景在 `SCENES` 中的索引。
pub const CAVE_SCENE: usize = 6;

/// 夜市场景在 `SCENES` 中的索引。
pub const NIGHTMARKET_SCENE: usize = 7;

/// 火车场景在 `SCENES` 中的索引。
pub const TRAIN_SCENE: usize = 8;

/// 暂停沉降时长 (视觉 500ms; 音频包络 300ms 见 ambient.rs, 两者独立)。
pub const SETTLE_DURATION: Duration = Duration::from_millis(500);

/// 动效沉降包络：计时运行 = 全量 (1), 暂停/空闲 = 0;
/// 目标变化触发 500ms 滑动，反向边沿从当前值续接 (无跳变)。
/// 与 `ambient::AmbientMixer` 的包络段同范式。
#[derive(Debug, Clone)]
pub struct MotionEnvelope {
    /// 包络当前值 (0..1, 1 = 全量雨效)。
    value: f32,
    /// 进行中的包络动画：(起始值，目标值，开始时刻)。
    anim: Option<(f32, f32, Duration)>,
    /// 上一帧见到的目标值 (边沿检测)。
    last_target: f32,
}

impl MotionEnvelope {
    /// 创建包络：初始 0 (无雨效), 等待首次 running 边沿淡入。
    pub fn new() -> Self {
        Self {
            value: 0.0,
            anim: None,
            last_target: 0.0,
        }
    }

    /// 推进包络并返回当前值 (0..=1)。
    ///
    /// 目标 = running ? 1 : 0; 目标变化触发 500ms 滑动动画，
    /// 动画进行中反向边沿从当前值续接 (无跳变)。
    pub fn gain(&mut self, running: bool, now: Duration) -> f32 {
        let target = if running { 1.0 } else { 0.0 };
        if target != self.last_target {
            self.anim = Some((self.value, target, now));
            self.last_target = target;
        }
        if let Some((start_v, target_v, start_t)) = self.anim {
            let t = (now.saturating_sub(start_t).as_secs_f32() / SETTLE_DURATION.as_secs_f32())
                .clamp(0.0, 1.0);
            self.value = start_v + (target_v - start_v) * t;
            if t >= 1.0 {
                self.anim = None;
            }
        }
        self.value
    }
}

impl Default for MotionEnvelope {
    fn default() -> Self {
        Self::new()
    }
}

/// 场景在交叉淡化中的权重：from 随 fade 淡出，to 随 fade 淡入，静止时恒 1。
fn scene_weight(scene: usize, from: usize, to: usize, fade: f32) -> f32 {
    let w = |idx: usize| if idx == scene { 1.0 } else { 0.0 };
    w(from) * (1.0 - fade) + w(to) * fade
}

/// 雨效强度合成：雨场景淡化权重 (不含包络)。
/// 2026-07-29 用户裁定：暂停时雨丝定格可见 — 包络不再决定雨的有无，
/// 只推进雨钟 (main.rs `rain_clock`, 暂停 500ms 减速冻结 / 恢复加速续走)。
pub fn rain_intensity(from: usize, to: usize, fade: f32) -> f32 {
    scene_weight(RAIN_SCENE, from, to, fade)
}

/// 火效强度合成：包络 × 篝火场景淡化权重。
pub fn fire_intensity(from: usize, to: usize, fade: f32, envelope: f32) -> f32 {
    envelope * scene_weight(BONFIRE_SCENE, from, to, fade)
}

/// 海效强度合成：包络 × 海场景淡化权重。
pub fn sea_intensity(from: usize, to: usize, fade: f32, envelope: f32) -> f32 {
    envelope * scene_weight(SEA_SCENE, from, to, fade)
}

/// 山效强度合成：包络 × 山场景淡化权重。
/// 山暂停时随既有 `MotionEnvelope` 500ms 归零，视觉逐像素回静态图
/// (径向光呼吸与山脊呼吸均随强度缩放归零); 不复用雨独立时钟范式。
pub fn mountain_intensity(from: usize, to: usize, fade: f32, envelope: f32) -> f32 {
    envelope * scene_weight(MOUNTAIN_SCENE, from, to, fade)
}

/// 森林效强度合成：包络 × 森林场景淡化权重。
/// 森林暂停时随既有 `MotionEnvelope` 500ms 归零，视觉逐像素回静态图
/// (顶光呼吸乘性归零、两道横雾 UV 漂移归零); 不复用雨独立时钟范式。
pub fn forest_intensity(from: usize, to: usize, fade: f32, envelope: f32) -> f32 {
    envelope * scene_weight(FOREST_SCENE, from, to, fade)
}

/// 铁匠铺效强度合成：包络 × 铁匠铺场景淡化权重。
/// 炉火呼吸 + 金属反光叠加。
pub fn blacksmith_intensity(from: usize, to: usize, fade: f32, envelope: f32) -> f32 {
    envelope * scene_weight(BLACKSMITH_SCENE, from, to, fade)
}

/// 洞穴效强度合成：包络 × 洞穴场景淡化权重。
pub fn cave_intensity(from: usize, to: usize, fade: f32, envelope: f32) -> f32 {
    envelope * scene_weight(CAVE_SCENE, from, to, fade)
}

/// 夜市效强度合成：包络 × 夜市场景淡化权重。
pub fn nightmarket_intensity(from: usize, to: usize, fade: f32, envelope: f32) -> f32 {
    envelope * scene_weight(NIGHTMARKET_SCENE, from, to, fade)
}

/// 火车效强度合成：包络 × 火车场景淡化权重。
pub fn train_intensity(from: usize, to: usize, fade: f32, envelope: f32) -> f32 {
    envelope * scene_weight(TRAIN_SCENE, from, to, fade)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenes::SCENES;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn rain_scene_index_points_at_rain() {
        assert_eq!(SCENES[RAIN_SCENE].name, "雨");
        // 雨场景唯一：其余场景不会被误判。
        assert_eq!(SCENES.iter().filter(|s| s.name == "雨").count(), 1);
    }

    #[test]
    fn bonfire_scene_index_points_at_bonfire() {
        assert_eq!(SCENES[BONFIRE_SCENE].name, "篝火");
        // 篝火场景唯一：其余场景不会被误判。
        assert_eq!(SCENES.iter().filter(|s| s.name == "篝火").count(), 1);
    }

    #[test]
    fn sea_scene_index_points_at_sea() {
        assert_eq!(SCENES[SEA_SCENE].name, "海");
        // 海场景唯一：其余场景不会被误判。
        assert_eq!(SCENES.iter().filter(|s| s.name == "海").count(), 1);
    }

    #[test]
    fn sea_intensity_weights_by_scene_and_fade() {
        // 海为 from: 随 fade 淡出。
        assert!((sea_intensity(SEA_SCENE, 0, 0.0, 1.0) - 1.0).abs() < 1e-6);
        assert!((sea_intensity(SEA_SCENE, 0, 0.5, 1.0) - 0.5).abs() < 1e-6);
        assert!(sea_intensity(SEA_SCENE, 0, 1.0, 1.0).abs() < 1e-6);
        // 海为 to: 随 fade 淡入。
        assert!((sea_intensity(0, SEA_SCENE, 0.5, 1.0) - 0.5).abs() < 1e-6);
        // 双非海：恒 0。
        assert_eq!(sea_intensity(0, 3, 0.5, 1.0), 0.0);
        // 静止于海 (from == to): 权重恒 1, 只随包络缩放。
        assert!((sea_intensity(SEA_SCENE, SEA_SCENE, 1.0, 0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn sea_coexists_with_rain_and_fire_on_crossfade() {
        // 海↔雨、海↔火交叉淡化中点：两效果各 0.5 并存 (标量并存，非互斥选择子)。
        assert!((sea_intensity(SEA_SCENE, RAIN_SCENE, 0.5, 1.0) - 0.5).abs() < 1e-6);
        assert!((rain_intensity(SEA_SCENE, RAIN_SCENE, 0.5) - 0.5).abs() < 1e-6);
        assert!((sea_intensity(BONFIRE_SCENE, SEA_SCENE, 0.5, 1.0) - 0.5).abs() < 1e-6);
        assert!((fire_intensity(BONFIRE_SCENE, SEA_SCENE, 0.5, 1.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn fire_intensity_weights_by_scene_and_fade() {
        // 篝火为 from: 随 fade 淡出。
        assert!((fire_intensity(BONFIRE_SCENE, 1, 0.0, 1.0) - 1.0).abs() < 1e-6);
        assert!((fire_intensity(BONFIRE_SCENE, 1, 0.5, 1.0) - 0.5).abs() < 1e-6);
        assert!(fire_intensity(BONFIRE_SCENE, 1, 1.0, 1.0).abs() < 1e-6);
        // 篝火为 to: 随 fade 淡入。
        assert!((fire_intensity(1, BONFIRE_SCENE, 0.5, 1.0) - 0.5).abs() < 1e-6);
        // 双非篝火：恒 0。
        assert_eq!(fire_intensity(1, 3, 0.5, 1.0), 0.0);
        // 静止于篝火 (from == to): 权重恒 1, 只随包络缩放。
        assert!((fire_intensity(BONFIRE_SCENE, BONFIRE_SCENE, 1.0, 0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn rain_and_fire_coexist_on_crossfade() {
        // 雨→篝火交叉淡化中点：两效果各 0.5 并存 (spec: 标量并存，非互斥选择子)。
        assert!((rain_intensity(RAIN_SCENE, BONFIRE_SCENE, 0.5) - 0.5).abs() < 1e-6);
        assert!((fire_intensity(RAIN_SCENE, BONFIRE_SCENE, 0.5, 1.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn envelope_idle_stays_zero() {
        let mut e = MotionEnvelope::new();
        assert_eq!(e.gain(false, ms(0)), 0.0);
        assert_eq!(e.gain(false, ms(10_000)), 0.0);
    }

    #[test]
    fn envelope_fades_in_over_500ms() {
        let mut e = MotionEnvelope::new();
        assert_eq!(e.gain(true, ms(0)), 0.0); // 边沿帧从 0 起
        assert!((e.gain(true, ms(250)) - 0.5).abs() < 1e-6);
        assert!((e.gain(true, ms(500)) - 1.0).abs() < 1e-6);
        assert!((e.gain(true, ms(9999)) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn envelope_pause_fades_out_and_resume_continues_from_current() {
        let mut e = MotionEnvelope::new();
        e.gain(true, ms(0));
        e.gain(true, ms(500)); // 全量
        // 暂停边沿：从 1 续接，不跳变。
        assert!((e.gain(false, ms(1000))) - 1.0 < 1e-6);
        // 淡出中点 (250ms) = 0.5。
        let mid = e.gain(false, ms(1250));
        assert!((mid - 0.5).abs() < 1e-6);
        // 淡出中恢复：从当前值续接淡入，不跳变; 500ms 后回全量。
        let v = e.gain(true, ms(1300));
        assert!((v - mid).abs() < 1e-6, "反向边沿应连续：{mid} -> {v}");
        assert!((e.gain(true, ms(1800)) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn rain_intensity_weights_by_scene_and_fade() {
        // 雨为 from: 随 fade 淡出。
        assert!((rain_intensity(RAIN_SCENE, 0, 0.0) - 1.0).abs() < 1e-6);
        assert!((rain_intensity(RAIN_SCENE, 0, 0.5) - 0.5).abs() < 1e-6);
        assert!(rain_intensity(RAIN_SCENE, 0, 1.0).abs() < 1e-6);
        // 雨为 to: 随 fade 淡入。
        assert!((rain_intensity(0, RAIN_SCENE, 0.5) - 0.5).abs() < 1e-6);
        // 双非雨：恒 0。
        assert_eq!(rain_intensity(0, 1, 0.5), 0.0);
        // 静止于雨 (from == to): 权重恒 1, 不随包络缩放 (暂停雨丝定格可见)。
        assert!((rain_intensity(RAIN_SCENE, RAIN_SCENE, 1.0) - 1.0).abs() < 1e-6);
    }

    // ---- 山、森林：SCENES[3] / SCENES[4] 索引锁 + intensity 并存语义 ----

    #[test]
    fn mountain_scene_index_points_at_mountain() {
        assert_eq!(SCENES[MOUNTAIN_SCENE].name, "山");
        // 山场景唯一：其余场景不会被误判。
        assert_eq!(SCENES.iter().filter(|s| s.name == "山").count(), 1);
    }

    #[test]
    fn forest_scene_index_points_at_forest() {
        assert_eq!(SCENES[FOREST_SCENE].name, "森林");
        // 森林场景唯一：其余场景不会被误判。
        assert_eq!(SCENES.iter().filter(|s| s.name == "森林").count(), 1);
    }

    #[test]
    fn mountain_intensity_weights_by_scene_and_fade() {
        // 山为 from: 随 fade 淡出。
        assert!((mountain_intensity(MOUNTAIN_SCENE, 1, 0.0, 1.0) - 1.0).abs() < 1e-6);
        assert!((mountain_intensity(MOUNTAIN_SCENE, 1, 0.5, 1.0) - 0.5).abs() < 1e-6);
        assert!(mountain_intensity(MOUNTAIN_SCENE, 1, 1.0, 1.0).abs() < 1e-6);
        // 山为 to: 随 fade 淡入。
        assert!((mountain_intensity(1, MOUNTAIN_SCENE, 0.5, 1.0) - 0.5).abs() < 1e-6);
        // 双非山：恒 0。
        assert_eq!(mountain_intensity(0, 1, 0.5, 1.0), 0.0);
        // 静止于山 (from == to): 权重恒 1, 与包络缩放 (与火/海一致; 与雨不同)。
        assert!((mountain_intensity(MOUNTAIN_SCENE, MOUNTAIN_SCENE, 1.0, 0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn forest_intensity_weights_by_scene_and_fade() {
        // 森林为 from: 随 fade 淡出。
        assert!((forest_intensity(FOREST_SCENE, 1, 0.0, 1.0) - 1.0).abs() < 1e-6);
        assert!((forest_intensity(FOREST_SCENE, 1, 0.5, 1.0) - 0.5).abs() < 1e-6);
        assert!(forest_intensity(FOREST_SCENE, 1, 1.0, 1.0).abs() < 1e-6);
        // 森林为 to: 随 fade 淡入。
        assert!((forest_intensity(1, FOREST_SCENE, 0.5, 1.0) - 0.5).abs() < 1e-6);
        // 双非森林：恒 0。
        assert_eq!(forest_intensity(0, 1, 0.5, 1.0), 0.0);
        // 静止于森林 (from == to): 权重恒 1, 与包络缩放 (与火/海一致)。
        assert!((forest_intensity(FOREST_SCENE, FOREST_SCENE, 1.0, 0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn mountain_and_forest_coexist_on_crossfade() {
        // 山↔森林交叉淡化中点：两效果各 0.5 并存 (标量并存，非互斥选择子)。
        assert!((mountain_intensity(MOUNTAIN_SCENE, FOREST_SCENE, 0.5, 1.0) - 0.5).abs() < 1e-6);
        assert!((forest_intensity(MOUNTAIN_SCENE, FOREST_SCENE, 0.5, 1.0) - 0.5).abs() < 1e-6);
        // 山 ↔ 雨 交叉淡化中点：山强度 0.5, 与雨并存 (后者例外但同标量纪律)。
        assert!((mountain_intensity(MOUNTAIN_SCENE, RAIN_SCENE, 0.5, 1.0) - 0.5).abs() < 1e-6);
        assert!((rain_intensity(MOUNTAIN_SCENE, RAIN_SCENE, 0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn mountain_intensity_pauses_fall_to_zero_in_500ms() {
        // 山沿用火/海"暂停回静态"语义: 包络 500ms 内从 1 滑到 0 时
        // mountain_intensity 同步缩放，视觉逐像素回静态图 (无独立 clock, 与雨相反)。
        let mut envelope = MotionEnvelope::new();
        envelope.gain(true, ms(0));
        // 全量：山强度跟随 envelope.
        let full = envelope.gain(true, ms(500));
        assert!((full - 1.0).abs() < 1e-6);
        assert!((mountain_intensity(MOUNTAIN_SCENE, MOUNTAIN_SCENE, 1.0, full) - 1.0).abs() < 1e-6);
        // 暂停：envelope 从 1 经 500ms 滑到 0.
        envelope.gain(false, ms(1000));
        let mid = envelope.gain(false, ms(1250));
        assert!((mid - 0.5).abs() < 1e-6);
        assert!((mountain_intensity(MOUNTAIN_SCENE, MOUNTAIN_SCENE, 1.0, mid) - 0.5).abs() < 1e-6);
        let zero = envelope.gain(false, ms(1600));
        assert!(zero.abs() < 1e-6);
        assert_eq!(
            mountain_intensity(MOUNTAIN_SCENE, MOUNTAIN_SCENE, 1.0, zero),
            0.0
        );
    }

    #[test]
    fn forest_intensity_pauses_fall_to_zero_in_500ms() {
        // 森林沿用火/海"暂停回静态"语义 (与 mountain 测试同构)。
        let mut envelope = MotionEnvelope::new();
        envelope.gain(true, ms(0));
        let full = envelope.gain(true, ms(500));
        assert!((full - 1.0).abs() < 1e-6);
        assert!((forest_intensity(FOREST_SCENE, FOREST_SCENE, 1.0, full) - 1.0).abs() < 1e-6);
        envelope.gain(false, ms(1000));
        let mid = envelope.gain(false, ms(1250));
        assert!((mid - 0.5).abs() < 1e-6);
        assert!((forest_intensity(FOREST_SCENE, FOREST_SCENE, 1.0, mid) - 0.5).abs() < 1e-6);
        let zero = envelope.gain(false, ms(1600));
        assert!(zero.abs() < 1e-6);
        assert_eq!(forest_intensity(FOREST_SCENE, FOREST_SCENE, 1.0, zero), 0.0);
    }

    // ---- 铁匠铺、洞穴、夜市、火车：SCENES[5..8] 索引锁 ----

    #[test]
    fn blacksmith_scene_index_points_at_blacksmith() {
        assert_eq!(SCENES[BLACKSMITH_SCENE].name, "铁匠铺");
        assert_eq!(SCENES.iter().filter(|s| s.name == "铁匠铺").count(), 1);
    }

    #[test]
    fn cave_scene_index_points_at_cave() {
        assert_eq!(SCENES[CAVE_SCENE].name, "洞穴");
        assert_eq!(SCENES.iter().filter(|s| s.name == "洞穴").count(), 1);
    }

    #[test]
    fn nightmarket_scene_index_points_at_nightmarket() {
        assert_eq!(SCENES[NIGHTMARKET_SCENE].name, "夜市");
        assert_eq!(SCENES.iter().filter(|s| s.name == "夜市").count(), 1);
    }

    #[test]
    fn train_scene_index_points_at_train() {
        assert_eq!(SCENES[TRAIN_SCENE].name, "火车");
        assert_eq!(SCENES.iter().filter(|s| s.name == "火车").count(), 1);
    }

    #[test]
    fn blacksmith_intensity_weights_by_scene_and_fade() {
        // 铁匠铺为 from: 随 fade 淡出。
        assert!((blacksmith_intensity(BLACKSMITH_SCENE, 0, 0.0, 1.0) - 1.0).abs() < 1e-6);
        assert!((blacksmith_intensity(BLACKSMITH_SCENE, 0, 0.5, 1.0) - 0.5).abs() < 1e-6);
        assert!(blacksmith_intensity(BLACKSMITH_SCENE, 0, 1.0, 1.0).abs() < 1e-6);
        // 铁匠铺为 to: 随 fade 淡入。
        assert!((blacksmith_intensity(0, BLACKSMITH_SCENE, 0.5, 1.0) - 0.5).abs() < 1e-6);
        // 双非铁匠铺：恒 0。
        assert_eq!(blacksmith_intensity(0, 1, 0.5, 1.0), 0.0);
        // 静止于铁匠铺 (from == to): 权重恒 1, 只随包络缩放。
        assert!(
            (blacksmith_intensity(BLACKSMITH_SCENE, BLACKSMITH_SCENE, 1.0, 0.5) - 0.5).abs() < 1e-6
        );
    }

    #[test]
    fn cave_intensity_weights_by_scene_and_fade() {
        assert!((cave_intensity(CAVE_SCENE, 0, 0.0, 1.0) - 1.0).abs() < 1e-6);
        assert!((cave_intensity(CAVE_SCENE, 0, 0.5, 1.0) - 0.5).abs() < 1e-6);
        assert!(cave_intensity(CAVE_SCENE, 0, 1.0, 1.0).abs() < 1e-6);
        assert!((cave_intensity(0, CAVE_SCENE, 0.5, 1.0) - 0.5).abs() < 1e-6);
        assert_eq!(cave_intensity(0, 1, 0.5, 1.0), 0.0);
    }

    #[test]
    fn nightmarket_intensity_weights_by_scene_and_fade() {
        assert!((nightmarket_intensity(NIGHTMARKET_SCENE, 0, 0.0, 1.0) - 1.0).abs() < 1e-6);
        assert!((nightmarket_intensity(NIGHTMARKET_SCENE, 0, 0.5, 1.0) - 0.5).abs() < 1e-6);
        assert!(nightmarket_intensity(NIGHTMARKET_SCENE, 0, 1.0, 1.0).abs() < 1e-6);
        assert!((nightmarket_intensity(0, NIGHTMARKET_SCENE, 0.5, 1.0) - 0.5).abs() < 1e-6);
        assert_eq!(nightmarket_intensity(0, 1, 0.5, 1.0), 0.0);
    }

    #[test]
    fn train_intensity_weights_by_scene_and_fade() {
        assert!((train_intensity(TRAIN_SCENE, 0, 0.0, 1.0) - 1.0).abs() < 1e-6);
        assert!((train_intensity(TRAIN_SCENE, 0, 0.5, 1.0) - 0.5).abs() < 1e-6);
        assert!(train_intensity(TRAIN_SCENE, 0, 1.0, 1.0).abs() < 1e-6);
        assert!((train_intensity(0, TRAIN_SCENE, 0.5, 1.0) - 0.5).abs() < 1e-6);
        assert_eq!(train_intensity(0, 1, 0.5, 1.0), 0.0);
    }
}
