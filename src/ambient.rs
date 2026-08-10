//! @author 十四叔
//! @date 2026/07/27

//! 场景环境音混音器 (纯逻辑)。
//!
//! 每帧把视觉淡化的 `(from, to, fade)` 与运行态转成两个音频槽的音量：
//! 音量 = 淡化权重 × 增益包络 × `AMBIENT_VOLUME`。
//! - 淡化权重：静止 (from == to, fade = 1) 时全量落在 to 槽;
//!   切换中按 fade 在 from/to 间分配，与画面 800ms 交叉淡化同源同步。
//! - 增益包络：目标增益 = running ? duck : 0 (暂停静音; 休息期 duck 沉降)。
//!   目标变化 (开始/暂停/相位切换) 触发 300ms 线性包络，反向边沿从当前值
//!   续接 (无跳变), 稳定态精确到达目标。
//!
//! 时间由外部注入，不读 wall-clock, 可完整单元测试。
//!
//! 下半部分为 rodio 输出适配层 (`AmbientPlayer`): 懒初始化输出流 +
//! from/to 双槽 `Player` (与视觉场景纹理 LRU 同构) + 静默降级。

use std::fs::File;
use std::io::BufReader;
use std::time::Duration;

use rodio::Source;

/// 场景音源路径 (与 `scenes::SCENES` 索引对齐)。
pub const SCENE_AUDIO: [&str; 9] = [
    "assets/audio/bonfire.ogg",
    "assets/audio/sea.ogg",
    "assets/audio/rain.ogg",
    "assets/audio/mountain.ogg",
    "assets/audio/forest.ogg",
    "assets/audio/blacksmith.ogg",
    "assets/audio/cave.ogg",
    "assets/audio/nightmarket.ogg",
    "assets/audio/train.ogg",
];

/// 环境音目标音量 (固定，无设置 UI)。
pub const AMBIENT_VOLUME: f32 = 0.6;

/// 休息期 (Break/LongBreak) 增益沉降系数：世界还在，但退远一步。
pub const BREAK_DUCK: f32 = 0.5;

/// 增益包络时长 (淡入/淡出/沉降/恢复对称)。
const ENVELOPE_DURATION: Duration = Duration::from_millis(300);

/// 环境音混音器：淡化权重 × 增益包络。
#[derive(Debug, Clone)]
pub struct AmbientMixer {
    /// 包络当前值 (0..1, 1 = 全量)。
    envelope: f32,
    /// 进行中的包络动画：(起始值，目标值，开始时刻)。
    anim: Option<(f32, f32, Duration)>,
    /// 上一帧见到的目标增益 (边沿检测：running 与 duck 合成)。
    last_target: f32,
    /// 全局环境音开关 (false = 目标恒 0, 静音所有场景音景)。
    enabled: bool,
}

impl AmbientMixer {
    /// 创建混音器：包络 0 (静音), 等待首次 running 边沿淡入; 环境音默认开。
    pub fn new() -> Self {
        Self {
            envelope: 0.0,
            anim: None,
            last_target: 0.0,
            enabled: true,
        }
    }

    /// 设置全局环境音开关 (false = 静音所有场景音景)。
    ///
    /// 关闭后目标增益恒 0 (与暂停同走 300ms 包络淡出), 恢复时从当前值平滑淡入。
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// 计算两槽音量：`[(from, v_from), (to, v_to)]`。
    ///
    /// `fade` 为视觉淡化进度 (0..1, 经缓动); `running` 为计时运行态;
    /// `duck` 为相位沉降系数 (Focus = 1.0, Break/LongBreak = [`BREAK_DUCK`])。
    /// 目标增益 = (enabled && running) ? duck : 0 — 环境音关闭时与暂停同走目标 0;
    /// 目标变化触发 300ms 包络动画，动画进行中反向边沿从当前值续接 (无跳变)。
    pub fn frame_volumes(
        &mut self,
        from: usize,
        to: usize,
        fade: f32,
        running: bool,
        duck: f32,
        now: Duration,
    ) -> [(usize, f32); 2] {
        let target = if self.enabled && running { duck } else { 0.0 };
        if target != self.last_target {
            self.anim = Some((self.envelope, target, now));
            self.last_target = target;
        }
        if let Some((start_v, target_v, start_t)) = self.anim {
            let t = (now.saturating_sub(start_t).as_secs_f32() / ENVELOPE_DURATION.as_secs_f32())
                .clamp(0.0, 1.0);
            self.envelope = start_v + (target_v - start_v) * t;
            if t >= 1.0 {
                self.anim = None;
            }
        }
        let gain = self.envelope * AMBIENT_VOLUME;
        [(from, (1.0 - fade) * gain), (to, fade * gain)]
    }
}

impl Default for AmbientMixer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// rodio 输出适配层
// ---------------------------------------------------------------------------

/// 环境音播放器：输出流 + from/to 双槽，消费 [`AmbientMixer`] 的帧音量。
///
/// - 懒初始化：首次出现非零音量才打开输出设备，启动路径 (Idle 静音) 不触音频。
/// - 双槽：槽位绑定场景，与视觉 `(from, to)` 纹理 LRU 同构;
///   淡化结束后旧场景槽自动释放，新场景槽按需重建 (`Decoder` 流式 + 无限循环)。
/// - 静默降级：打开设备失败永久降级 (`disabled`); 单条音源打不开记入
///   `failed_scenes` 不再重试。所有失败仅 `log::warn`, 不 panic。
pub struct AmbientPlayer {
    /// 输出流 (懒初始化; None = 尚未打开设备)。
    stream: Option<rodio::MixerDeviceSink>,
    /// 双槽：(绑定场景索引，播放器)。drop 即停播。
    slots: [Option<(usize, rodio::Player)>; 2],
    /// 永久降级旗标：输出设备打开失败后置位，之后每帧直接返回。
    disabled: bool,
    /// 打不开 (缺文件 / 解码失败) 的场景，避免 60fps 重试刷日志。
    failed_scenes: [bool; SCENE_AUDIO.len()],
}

impl AmbientPlayer {
    /// 创建播放器：未初始化，未降级，双槽为空。
    pub fn new() -> Self {
        Self {
            stream: None,
            slots: [None, None],
            disabled: false,
            failed_scenes: [false; SCENE_AUDIO.len()],
        }
    }

    /// 每帧应用混音结果：对齐槽位与活跃场景，设置两槽音量。
    ///
    /// 全静音且无活动槽时直接返回 (不开设备); 任一步失败仅 warn 不 panic。
    pub fn apply(&mut self, frame: [(usize, f32); 2]) {
        if self.disabled {
            return;
        }
        // 启动 Idle: 无音量且无槽，不触碰音频设备。
        let idle = frame.iter().all(|(_, v)| *v <= 0.0) && self.slots.iter().all(Option::is_none);
        if idle {
            return;
        }
        if self.stream.is_none() {
            match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(stream) => self.stream = Some(stream),
                Err(err) => {
                    log::warn!("环境音输出设备打开失败，永久降级：{err}");
                    self.disabled = true;
                    return;
                }
            }
        }
        let Some(stream) = self.stream.as_ref() else {
            return;
        };
        let active = [frame[0].0, frame[1].0];
        // 释放不再活跃的槽 (淡化完成后旧 from 退场)。
        for slot in &mut self.slots {
            if let Some((scene, _)) = slot {
                if !active.contains(scene) {
                    *slot = None;
                }
            }
        }
        // 活跃场景缺槽时绑定到空槽; 已知失败的场景跳过。
        for (scene, _) in frame.iter().copied() {
            if scene >= SCENE_AUDIO.len()
                || self.failed_scenes[scene]
                || self.slots.iter().flatten().any(|(s, _)| *s == scene)
            {
                continue;
            }
            let Some(slot) = self.slots.iter_mut().find(|slot| slot.is_none()) else {
                continue;
            };
            match Self::build_player(stream, scene) {
                Some(player) => *slot = Some((scene, player)),
                None => self.failed_scenes[scene] = true,
            }
        }
        // 音量每帧直写 (300ms 包络 / 800ms 淡化都由 mixer 算好)。
        // 全零音量时暂停槽位：常驻应用的支配状态是暂停，不空转解码/重采样。
        // play/pause 仅写 AtomicBool, 每帧调用无负担; 音量回正即续播 (位置保持)。
        for (scene, volume) in frame {
            if let Some((_, player)) = self.slots.iter().flatten().find(|(s, _)| *s == scene) {
                if volume > 0.0 {
                    player.play();
                } else {
                    player.pause();
                }
                player.set_volume(volume);
            }
        }
    }

    /// 为场景构建循环播放槽：打开文件 + 流式解码 + 无限循环。
    fn build_player(stream: &rodio::MixerDeviceSink, scene: usize) -> Option<rodio::Player> {
        let path = SCENE_AUDIO[scene];
        let source = match LoopingDecoder::new(path) {
            Some(source) => source,
            None => {
                log::warn!("环境音打开/解码失败 ({path})");
                return None;
            }
        };
        let player = rodio::Player::connect_new(stream.mixer());
        player.append(source);
        Some(player)
    }
}

impl Default for AmbientPlayer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl AmbientPlayer {
    /// 测试辅助：强制永久降级，避免 tick 路径触碰真实音频设备。
    pub fn disable_for_test(&mut self) {
        self.disabled = true;
    }
}

/// 无限循环的流式解码源：当前解码器耗尽时重开文件从头解码续播。
///
/// 存在理由：rodio 0.22 的 `repeat_infinite` 内部走 `buffered()`, 建缓冲时
/// 把 symphonia 解码器初始空包 (`current_span_len() == Some(0)`) 误判为流结束，
/// 追加后整源秒空、无声。此处自实现循环绕开该环节;
/// 文件首尾已做 50ms 微 crossfade, 回卷无接缝爆音。
///
/// 回卷不用 `try_seek`: symphonia 粗粒度 seek 回 0 会跳过首个 Vorbis 包
/// (mountain 实测少 1156 采样 ≈ 24ms), 每循环一次接缝就爆音一声;
/// 重开文件从头解码才是逐位一致的真回卷 (小文件 probe 开销可忽略)。
struct LoopingDecoder {
    /// 音源路径 (重开文件兜底用)。
    path: &'static str,
    /// 当前解码器; None 表示已永久失败 (静默降级，后续 next 一律 None)。
    current: Option<rodio::Decoder<BufReader<File>>>,
    /// 声道数 (自首帧捕获，循环不变)。
    channels: rodio::ChannelCount,
    /// 采样率 (自首帧捕获，循环不变)。
    sample_rate: rodio::SampleRate,
}

impl LoopingDecoder {
    /// 打开并解码首轮; 失败返回 None (调用方记 failed_scenes)。
    fn new(path: &'static str) -> Option<Self> {
        let decoder = Self::decode(path)?;
        Some(Self {
            path,
            channels: decoder.channels(),
            sample_rate: decoder.sample_rate(),
            current: Some(decoder),
        })
    }

    /// 打开文件并创建解码器; 任一步失败返回 None。
    fn decode(path: &str) -> Option<rodio::Decoder<BufReader<File>>> {
        let file = File::open(path).ok()?;
        rodio::Decoder::new(BufReader::new(file)).ok()
    }
}

impl Iterator for LoopingDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        // 最多两轮：当前解码器取流 → 耗尽则重开文件回卷再取; 仍无采样视为永久失败。
        for _ in 0..2 {
            let mut decoder = self.current.take()?;
            if let Some(sample) = decoder.next() {
                self.current = Some(decoder);
                return Some(sample);
            }
            // 耗尽：重开文件回卷 (不用 try_seek, 见结构体文档)。
            self.current = Self::decode(self.path);
        }
        // 回卷后仍取不到采样 (文件损坏等): 永久关闭，防音频线程空转。
        log::warn!("环境音循环源永久关闭 ({})", self.path);
        self.current = None;
        None
    }
}

impl Source for LoopingDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None // 无限流
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.channels
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None // 无限循环
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenes::SCENES;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn scene_audio_array_aligns_with_scenes() {
        assert_eq!(SCENE_AUDIO.len(), SCENES.len(), "音源数组应与场景一一对应");
    }

    #[test]
    fn idle_stays_silent() {
        let mut m = AmbientMixer::new();
        for t in [0, 100, 10_000] {
            let v = m.frame_volumes(0, 0, 1.0, false, 1.0, ms(t));
            assert_eq!(v, [(0, 0.0), (0, 0.0)], "Idle 应始终静音 (t={t})");
        }
    }

    #[test]
    fn running_fades_in_over_300ms() {
        let mut m = AmbientMixer::new();
        // 边沿帧：包络从 0 起，音量为 0。
        let v = m.frame_volumes(0, 0, 1.0, true, 1.0, ms(1000));
        assert_eq!(v[1].1, 0.0);
        // 中点：包络 0.5 → 音量 0.3。
        let v = m.frame_volumes(0, 0, 1.0, true, 1.0, ms(1150));
        assert!((v[1].1 - 0.3).abs() < 1e-6, "中点应为半量：{}", v[1].1);
        // 终点及之后：稳定全量 0.6。
        let v = m.frame_volumes(0, 0, 1.0, true, 1.0, ms(1300));
        assert!((v[1].1 - AMBIENT_VOLUME).abs() < 1e-6);
        let v = m.frame_volumes(0, 0, 1.0, true, 1.0, ms(999_999));
        assert!((v[1].1 - AMBIENT_VOLUME).abs() < 1e-6);
    }

    #[test]
    fn pause_fades_out_over_300ms() {
        let mut m = AmbientMixer::new();
        m.frame_volumes(0, 0, 1.0, true, 1.0, ms(0));
        m.frame_volumes(0, 0, 1.0, true, 1.0, ms(300)); // 淡入完成，全量
        // 暂停边沿：从全量起淡。
        let v = m.frame_volumes(0, 0, 1.0, false, 1.0, ms(1000));
        assert!((v[1].1 - AMBIENT_VOLUME).abs() < 1e-6, "边沿帧应连续");
        let v = m.frame_volumes(0, 0, 1.0, false, 1.0, ms(1150));
        assert!((v[1].1 - 0.3).abs() < 1e-6, "中点应为半量：{}", v[1].1);
        let v = m.frame_volumes(0, 0, 1.0, false, 1.0, ms(1300));
        assert_eq!(v[1].1, 0.0);
    }

    #[test]
    fn fade_interpolation_splits_volume() {
        let mut m = AmbientMixer::new();
        m.frame_volumes(0, 0, 1.0, true, 1.0, ms(0));
        m.frame_volumes(0, 0, 1.0, true, 1.0, ms(300)); // 全量
        // 切换起点：全量在 from。
        let v = m.frame_volumes(0, 1, 0.0, true, 1.0, ms(400));
        assert!((v[0].1 - AMBIENT_VOLUME).abs() < 1e-6);
        assert_eq!(v[1].1, 0.0);
        // 中点：两槽各半。
        let v = m.frame_volumes(0, 1, 0.5, true, 1.0, ms(500));
        assert!((v[0].1 - 0.3).abs() < 1e-6);
        assert!((v[1].1 - 0.3).abs() < 1e-6);
        // 终点：全量在 to。
        let v = m.frame_volumes(0, 1, 1.0, true, 1.0, ms(600));
        assert_eq!(v[0].1, 0.0);
        assert!((v[1].1 - AMBIENT_VOLUME).abs() < 1e-6);
    }

    #[test]
    fn envelope_and_fade_are_independent() {
        let mut m = AmbientMixer::new();
        m.frame_volumes(0, 0, 1.0, true, 1.0, ms(0));
        // 淡化中点 + 包络中点 (150ms): 音量 = 0.5 淡化 × 0.5 包络 × 0.6 = 0.15。
        let v = m.frame_volumes(0, 1, 0.5, true, 1.0, ms(150));
        assert!((v[0].1 - 0.15).abs() < 1e-6, "from: {}", v[0].1);
        assert!((v[1].1 - 0.15).abs() < 1e-6, "to: {}", v[1].1);
    }

    #[test]
    fn retrigger_mid_envelope_continues_from_current_value() {
        let mut m = AmbientMixer::new();
        m.frame_volumes(0, 0, 1.0, true, 1.0, ms(0));
        m.frame_volumes(0, 0, 1.0, true, 1.0, ms(300)); // 全量
        m.frame_volumes(0, 0, 1.0, false, 1.0, ms(1000)); // 开始淡出
        let v = m.frame_volumes(0, 0, 1.0, false, 1.0, ms(1150)); // 淡出中点 0.3
        let mid = v[1].1;
        // 淡出中点恢复：从当前包络值 (0.5) 续接淡入，不跳变。
        let v = m.frame_volumes(0, 0, 1.0, true, 1.0, ms(1200));
        assert!(
            (v[1].1 - mid).abs() < 1e-6,
            "反向边沿应连续：{mid} -> {}",
            v[1].1
        );
        // 固定 300ms 包络时长：中点 (150ms) 走到 0.75 → 0.45; 终点 (300ms) 回全量。
        let v = m.frame_volumes(0, 0, 1.0, true, 1.0, ms(1350));
        assert!((v[1].1 - 0.45).abs() < 1e-6, "中点：{}", v[1].1);
        let v = m.frame_volumes(0, 0, 1.0, true, 1.0, ms(1500));
        assert!((v[1].1 - AMBIENT_VOLUME).abs() < 1e-6);
    }

    #[test]
    fn restored_running_session_fades_in_from_silence() {
        // 恢复 Running 会话：首帧即 running=true, 从静音淡入而非爆音。
        let mut m = AmbientMixer::new();
        let v = m.frame_volumes(2, 2, 1.0, true, 1.0, ms(0));
        assert_eq!(v[1].1, 0.0);
        let v = m.frame_volumes(2, 2, 1.0, true, 1.0, ms(300));
        assert!((v[1].1 - AMBIENT_VOLUME).abs() < 1e-6);
    }

    #[test]
    fn break_duck_glides_to_half_volume() {
        let mut m = AmbientMixer::new();
        m.frame_volumes(0, 0, 1.0, true, 1.0, ms(0));
        m.frame_volumes(0, 0, 1.0, true, 1.0, ms(300)); // 全量
        // 进入休息：duck 0.5, 300ms 滑到半量。
        let v = m.frame_volumes(0, 0, 1.0, true, BREAK_DUCK, ms(400));
        assert!((v[1].1 - AMBIENT_VOLUME).abs() < 1e-6, "边沿帧应连续");
        let v = m.frame_volumes(0, 0, 1.0, true, BREAK_DUCK, ms(550));
        assert!((v[1].1 - 0.45).abs() < 1e-6, "中点应为 0.45: {}", v[1].1);
        let v = m.frame_volumes(0, 0, 1.0, true, BREAK_DUCK, ms(700));
        let half = AMBIENT_VOLUME * BREAK_DUCK;
        assert!(
            (v[1].1 - half).abs() < 1e-6,
            "终点应为半量 {half}: {}",
            v[1].1
        );
        // 稳定在半量。
        let v = m.frame_volumes(0, 0, 1.0, true, BREAK_DUCK, ms(999_999));
        assert!((v[1].1 - half).abs() < 1e-6);
    }

    #[test]
    fn paused_during_break_stays_silent() {
        // 休息期暂停：目标 = 0 (duck 不抬升), 保持静音。
        let mut m = AmbientMixer::new();
        for t in [0, 100, 10_000] {
            let v = m.frame_volumes(0, 0, 1.0, false, BREAK_DUCK, ms(t));
            assert_eq!(v[1].1, 0.0, "休息期暂停应静音 (t={t})");
        }
    }

    #[test]
    fn muted_scene_audio_stays_silent_while_running() {
        // 全局环境音开关关闭: 即使计时运行, 目标恒 0, 包络不抬升。
        let mut m = AmbientMixer::new();
        m.set_enabled(false);
        for t in [0, 100, 10_000] {
            let v = m.frame_volumes(0, 0, 1.0, true, 1.0, ms(t));
            assert_eq!(v[1].1, 0.0, "静音时应始终无声 (t={t})");
        }
    }

    #[test]
    fn unmute_fades_back_in_over_300ms() {
        // 静音运行 → 开声: 从 0 平滑淡入 300ms, 无跳变爆音。
        let mut m = AmbientMixer::new();
        m.set_enabled(false);
        m.frame_volumes(0, 0, 1.0, true, 1.0, ms(0));
        m.frame_volumes(0, 0, 1.0, true, 1.0, ms(300));
        m.set_enabled(true);
        let v = m.frame_volumes(0, 0, 1.0, true, 1.0, ms(1000));
        assert_eq!(v[1].1, 0.0, "开声边沿帧应连续");
        let v = m.frame_volumes(0, 0, 1.0, true, 1.0, ms(1150));
        assert!((v[1].1 - 0.3).abs() < 1e-6, "中点应为 0.3: {}", v[1].1);
        let v = m.frame_volumes(0, 0, 1.0, true, 1.0, ms(1300));
        assert!((v[1].1 - AMBIENT_VOLUME).abs() < 1e-6, "终点应回全量");
    }

    #[test]
    fn mute_mid_session_fades_out_smoothly() {
        // 运行全量中关声: 300ms 平滑淡出, 无跳变 (边沿帧从当前值续接)。
        let mut m = AmbientMixer::new();
        m.frame_volumes(0, 0, 1.0, true, 1.0, ms(0));
        m.frame_volumes(0, 0, 1.0, true, 1.0, ms(300)); // 全量
        m.set_enabled(false);
        let v = m.frame_volumes(0, 0, 1.0, true, 1.0, ms(1000));
        assert!((v[1].1 - AMBIENT_VOLUME).abs() < 1e-6, "关声边沿帧应连续");
        let v = m.frame_volumes(0, 0, 1.0, true, 1.0, ms(1150));
        assert!((v[1].1 - 0.3).abs() < 1e-6, "中点应为半量: {}", v[1].1);
        let v = m.frame_volumes(0, 0, 1.0, true, 1.0, ms(1300));
        assert_eq!(v[1].1, 0.0);
    }

    #[test]
    fn return_to_focus_restores_full_volume() {
        let mut m = AmbientMixer::new();
        m.frame_volumes(0, 0, 1.0, true, 1.0, ms(0));
        m.frame_volumes(0, 0, 1.0, true, BREAK_DUCK, ms(300)); // 全量→duck 边沿
        let v = m.frame_volumes(0, 0, 1.0, true, BREAK_DUCK, ms(600)); // 到半量
        assert!((v[1].1 - AMBIENT_VOLUME * BREAK_DUCK).abs() < 1e-6);
        // 回专注：300ms 滑回全量。
        let v = m.frame_volumes(0, 0, 1.0, true, 1.0, ms(700));
        assert!(
            (v[1].1 - AMBIENT_VOLUME * BREAK_DUCK).abs() < 1e-6,
            "边沿帧应连续"
        );
        let v = m.frame_volumes(0, 0, 1.0, true, 1.0, ms(1000));
        assert!((v[1].1 - AMBIENT_VOLUME).abs() < 1e-6, "终点应回全量");
    }

    #[test]
    fn player_idle_apply_does_not_touch_device() {
        // 全静音 + 空槽：apply 直接返回，不开输出设备，不降级。
        let mut player = AmbientPlayer::new();
        player.apply([(0, 0.0), (0, 0.0)]);
        assert!(player.stream.is_none(), "Idle 不应打开输出设备");
        assert!(!player.disabled);
    }

    #[test]
    fn player_failed_scene_is_skipped_on_apply() {
        // 已知失败的场景：apply 跳过绑定，不建槽不降级。
        let mut player = AmbientPlayer::new();
        player.failed_scenes[4] = true;
        player.apply([(4, 0.0), (4, 0.0)]);
        assert!(player.slots.iter().all(Option::is_none));
        assert!(!player.disabled);
    }

    #[test]
    fn scene_audio_files_decode_as_ogg_vorbis() {
        // 解码冒烟：验证 rodio 精简特性 (symphonia-ogg + symphonia-vorbis)
        // 足以解码 5 条资产; 不触输出设备，纯解码路径。
        for path in SCENE_AUDIO {
            let file = File::open(path).unwrap_or_else(|e| panic!("{path}: {e}"));
            let decoder = rodio::Decoder::new(BufReader::new(file))
                .unwrap_or_else(|e| panic!("{path} 解码失败：{e}"));
            let total = decoder.take(4096).count();
            assert!(total > 0, "{path} 应能解出采样");
        }
    }

    #[test]
    fn looping_decoder_keeps_producing_beyond_single_pass() {
        // 回归：rodio 0.22 repeat_infinite 对 symphonia 解码器秒空的 bug。
        // bonfire.ogg 单遍约 12.5s; 拉 300 万采样 (≈2.7 遍) 必须全部有值。
        let source = LoopingDecoder::new(SCENE_AUDIO[0]).expect("bonfire.ogg 应可解码");
        const PULL: usize = 3_000_000;
        let produced = source.take(PULL).count();
        assert_eq!(produced, PULL, "循环源应在单遍耗尽后无缝回卷续播");
    }

    #[test]
    fn looping_decoder_preserves_format() {
        let fresh = {
            let file = File::open(SCENE_AUDIO[0]).unwrap();
            rodio::Decoder::new(BufReader::new(file)).unwrap()
        };
        let source = LoopingDecoder::new(SCENE_AUDIO[0]).unwrap();
        assert_eq!(source.channels(), fresh.channels());
        assert_eq!(source.sample_rate(), fresh.sample_rate());
        assert_eq!(source.total_duration(), None, "无限循环不报时长");
        assert_eq!(source.current_span_len(), None, "无限流不报 span 长度");
    }

    #[test]
    fn looping_decoder_restart_is_sample_accurate() {
        // 回归: 回卷必须落在流的第 0 采样。symphonia 粗粒度 try_seek(0) 会跳过
        // 首个 Vorbis 包 (mountain 实测 1156 采样 ≈ 24ms), 每循环一次接缝爆音。
        // 单遍采样数动态测量 (symphonia 与 ffmpeg 端点修剪略有差异, 不硬编码)。
        let pass = LoopingDecoder::decode(SCENE_AUDIO[3])
            .expect("mountain.ogg 应可解码")
            .count();
        let all: Vec<f32> = LoopingDecoder::new(SCENE_AUDIO[3])
            .expect("mountain.ogg 应可解码")
            .take(pass * 2)
            .collect();
        assert_eq!(all.len(), pass * 2, "应能拉满两遍");
        let first_diff = (0..pass).find(|&i| all[i] != all[pass + i]);
        assert_eq!(first_diff, None, "第二遍应与第一遍逐位一致 (回卷位置准确)");
    }

    #[test]
    fn looping_decoder_missing_file_degrades_to_none() {
        assert!(LoopingDecoder::new("assets/audio/does-not-exist.ogg").is_none());
    }
}
