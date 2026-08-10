//! @author 十四叔
//! @date 2026/07/23

//! 场景资产与调色板常量 —— 由 tools/export-scenes.py 生成, 勿手改。

use danqing::{Color, ScenePalette, SceneSpec};

/// POC 场景清单 (数组顺序即 ◀/▶ 切换顺序)。
pub const SCENES: [SceneSpec; 9] = [
    SceneSpec {
        name: "篝火",
        image: "assets/scenes/bonfire.png",
        palette: ScenePalette {
            base: Color::from_srgb8(26, 15, 10),
            accent: Color::from_srgb8(255, 159, 67),
            text_primary: Color::from_srgb8(240, 230, 215),
            text_secondary: Color::from_srgb8(195, 180, 165),
            surface: Color::rgba(0.0, 0.0, 0.0, 0.25),
            surface_input: Color::rgba(0.0, 0.0, 0.0, 0.38),
            backdrop_light: Color::from_srgb8(208, 217, 228),
            backdrop_dark: Color::from_srgb8(8, 14, 12),
        },
    },
    SceneSpec {
        name: "海",
        image: "assets/scenes/sea.png",
        palette: ScenePalette {
            base: Color::from_srgb8(168, 221, 232),
            accent: Color::from_srgb8(15, 55, 75),
            text_primary: Color::from_srgb8(255, 255, 255),
            text_secondary: Color::from_srgb8(200, 210, 220),
            surface: Color::rgba(0.0, 0.0, 0.0, 0.25),
            surface_input: Color::rgba(0.0, 0.0, 0.0, 0.38),
            backdrop_light: Color::from_srgb8(255, 235, 204),
            backdrop_dark: Color::from_srgb8(0, 16, 16),
        },
    },
    SceneSpec {
        name: "雨",
        image: "assets/scenes/rain.png",
        palette: ScenePalette {
            base: Color::from_srgb8(82, 95, 107),
            accent: Color::from_srgb8(127, 179, 217),
            text_primary: Color::from_srgb8(242, 246, 249),
            text_secondary: Color::from_srgb8(195, 205, 213),
            surface: Color::rgba(0.0, 0.0, 0.0, 0.25),
            surface_input: Color::rgba(0.0, 0.0, 0.0, 0.38),
            backdrop_light: Color::from_srgb8(193, 207, 210),
            backdrop_dark: Color::from_srgb8(25, 40, 37),
        },
    },
    SceneSpec {
        name: "山",
        image: "assets/scenes/mountain.png",
        palette: ScenePalette {
            base: Color::from_srgb8(86, 80, 115),
            accent: Color::from_srgb8(232, 192, 122),
            text_primary: Color::from_srgb8(245, 241, 250),
            text_secondary: Color::from_srgb8(205, 198, 218),
            surface: Color::rgba(0.0, 0.0, 0.0, 0.25),
            surface_input: Color::rgba(0.0, 0.0, 0.0, 0.38),
            backdrop_light: Color::from_srgb8(170, 130, 147),
            backdrop_dark: Color::from_srgb8(1, 19, 27),
        },
    },
    SceneSpec {
        name: "森林",
        image: "assets/scenes/forest.png",
        palette: ScenePalette {
            base: Color::from_srgb8(50, 72, 59),
            accent: Color::from_srgb8(172, 198, 158),
            text_primary: Color::from_srgb8(240, 246, 240),
            text_secondary: Color::from_srgb8(186, 201, 187),
            surface: Color::rgba(0.0, 0.0, 0.0, 0.25),
            surface_input: Color::rgba(0.0, 0.0, 0.0, 0.38),
            backdrop_light: Color::from_srgb8(169, 202, 193),
            backdrop_dark: Color::from_srgb8(0, 9, 4),
        },
    },
    SceneSpec {
        name: "铁匠铺",
        image: "assets/scenes/blacksmith.png",
        palette: ScenePalette {
            base: Color::from_srgb8(45, 25, 18),
            accent: Color::from_srgb8(255, 120, 40),
            text_primary: Color::from_srgb8(255, 235, 210),
            text_secondary: Color::from_srgb8(200, 175, 155),
            surface: Color::rgba(0.0, 0.0, 0.0, 0.25),
            surface_input: Color::rgba(0.0, 0.0, 0.0, 0.38),
            backdrop_light: Color::from_srgb8(252, 235, 158),
            backdrop_dark: Color::from_srgb8(0, 10, 17),
        },
    },
    SceneSpec {
        name: "洞穴",
        image: "assets/scenes/cave.png",
        palette: ScenePalette {
            base: Color::from_srgb8(20, 45, 55),
            accent: Color::from_srgb8(80, 200, 220),
            text_primary: Color::from_srgb8(220, 245, 250),
            text_secondary: Color::from_srgb8(160, 195, 205),
            surface: Color::rgba(0.0, 0.0, 0.0, 0.25),
            surface_input: Color::rgba(0.0, 0.0, 0.0, 0.38),
            backdrop_light: Color::from_srgb8(240, 254, 246),
            backdrop_dark: Color::from_srgb8(6, 26, 31),
        },
    },
    SceneSpec {
        name: "夜市",
        image: "assets/scenes/nightmarket.png",
        palette: ScenePalette {
            base: Color::from_srgb8(60, 30, 20),
            accent: Color::from_srgb8(255, 180, 50),
            text_primary: Color::from_srgb8(255, 245, 220),
            text_secondary: Color::from_srgb8(210, 185, 160),
            surface: Color::rgba(0.0, 0.0, 0.0, 0.25),
            surface_input: Color::rgba(0.0, 0.0, 0.0, 0.38),
            backdrop_light: Color::from_srgb8(254, 255, 255),
            backdrop_dark: Color::from_srgb8(34, 30, 23),
        },
    },
    SceneSpec {
        name: "火车",
        image: "assets/scenes/train.png",
        palette: ScenePalette {
            base: Color::from_srgb8(50, 45, 55),
            accent: Color::from_srgb8(200, 180, 140),
            text_primary: Color::from_srgb8(240, 235, 225),
            text_secondary: Color::from_srgb8(185, 178, 170),
            surface: Color::rgba(0.0, 0.0, 0.0, 0.25),
            surface_input: Color::rgba(0.0, 0.0, 0.0, 0.38),
            backdrop_light: Color::from_srgb8(230, 218, 190),
            backdrop_dark: Color::from_srgb8(11, 16, 19),
        },
    },
];

#[cfg(test)]
mod tests {
    //! 对比度护栏: 与 tools/export-scenes.py 生成期护栏同规则,
    //! 防止 scenes.rs 被手改后护栏静默失效 (spec: 大字 ≥3:1, 控件 ≥4:1)。
    use super::*;
    use danqing::{composite_over, contrast_ratio};

    /// 大字 (倒计时) 对场景背景极值的最低对比度。
    const DISPLAY_MIN: f32 = 3.0;
    /// 控件文字对玻璃合成底的最低对比度。
    const CONTROL_MIN: f32 = 4.0;

    #[test]
    fn all_scenes_pass_contrast_guards() {
        assert_eq!(SCENES.len(), 9, "沉浸世界应有 9 个场景");
        for spec in &SCENES {
            let p = &spec.palette;
            for (label, backdrop) in [
                ("backdrop_light", p.backdrop_light),
                ("backdrop_dark", p.backdrop_dark),
            ] {
                let display = contrast_ratio(p.text_primary, backdrop);
                assert!(
                    display >= DISPLAY_MIN,
                    "{}: 大字 vs {label} = {display:.2} < {DISPLAY_MIN}",
                    spec.name
                );
                let glass = composite_over(p.surface, backdrop);
                let control = contrast_ratio(p.text_primary, glass);
                assert!(
                    control >= CONTROL_MIN,
                    "{}: 控件文字 vs 玻璃({label}) = {control:.2} < {CONTROL_MIN}",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn scene_images_are_unique_and_named() {
        for (i, a) in SCENES.iter().enumerate() {
            for b in &SCENES[i + 1..] {
                assert_ne!(a.image, b.image, "场景图路径不应重复");
                assert_ne!(a.name, b.name, "场景名不应重复");
            }
        }
    }
}
