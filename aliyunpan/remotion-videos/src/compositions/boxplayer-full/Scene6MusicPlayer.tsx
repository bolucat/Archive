import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate, spring, useVideoConfig } from "remotion";

// ============ EDITABLE CONSTANTS ============
const BRAND = {
  bg: "#0B0E1A",
  surface: "#141828",
  text: "#F1F5F9",
  accent: "#00E3D5",
  accent2: "#EC4899",
  accent3: "#A855F7",
  muted: "#94A3B8",
  border: "#1E293B",
};

const SECTION_TITLE = "音乐粒子播放器";
const SECTION_SUBTITLE = "实时频谱可视化 · 逐字卡拉OK歌词 · 10段EQ";

const FEATURES = [
  { title: "实时频谱可视化", desc: "AudioContext 引擎 · 粒子动画背景", color: BRAND.accent },
  { title: "逐字卡拉OK歌词", desc: "翻译 / 罗马音双行 · 桌面浮动歌词", color: BRAND.accent2 },
  { title: "10段EQ + 混响", desc: "声像 · 变调不变速 · 耳机调音", color: BRAND.accent3 },
  { title: "多源歌词补全", desc: "网易云 / 酷狗 / QQ音乐 / 酷我 / 咪咕", color: "#F59E0B" },
];

const THEMES = ["深邃蓝", "暗夜紫", "赛博绿", "日落橙", "极光粉"];
// ============================================

export const Scene6MusicPlayer: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const titleOpacity = interpolate(frame, [0, 20], [0, 1], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  // Animated equalizer bars
  const bars = Array.from({ length: 24 }, (_, i) => {
    const phase = frame * 0.15 + i * 0.5;
    const baseHeight = 30 + Math.sin(phase) * 20 + Math.sin(phase * 2.3) * 15;
    const maxHeight = 80 + Math.abs(Math.sin(phase * 1.7)) * 60;
    return Math.min(baseHeight + Math.abs(Math.sin(phase * 0.8)) * 50, maxHeight);
  });

  // Feature cards
  const featStart = 50;
  const featStagger = 18;

  // Theme pills
  const themeStart = 130;

  // Exit
  const exitOpacity = interpolate(frame, [190, 210], [1, 0], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  return (
    <AbsoluteFill
      style={{
        background: `linear-gradient(180deg, ${BRAND.surface} 0%, ${BRAND.bg} 100%)`,
        justifyContent: "center",
        alignItems: "center",
        opacity: exitOpacity,
      }}
    >
      <div style={{ width: 1400, textAlign: "center" }}>
        {/* Badge */}
        <div
          style={{
            display: "inline-block",
            padding: "6px 20px",
            borderRadius: 16,
            background: `${BRAND.accent2}1A`,
            border: `1px solid ${BRAND.accent2}44`,
            color: BRAND.accent2,
            fontSize: 20,
            fontWeight: 600,
            marginBottom: 16,
            opacity: titleOpacity,
          }}
        >
          🎵 粒子播放器
        </div>

        <h2
          style={{
            fontSize: 52,
            fontWeight: 800,
            color: BRAND.text,
            opacity: titleOpacity,
            margin: "0 0 8px",
          }}
        >
          {SECTION_TITLE}
        </h2>
        <p
          style={{
            fontSize: 26,
            color: BRAND.muted,
            opacity: titleOpacity,
            margin: "0 0 32px",
          }}
        >
          {SECTION_SUBTITLE}
        </p>

        {/* Visualizer */}
        <div
          style={{
            display: "flex",
            alignItems: "flex-end",
            justifyContent: "center",
            gap: 6,
            height: 140,
            marginBottom: 36,
            opacity: interpolate(frame, [15, 40], [0, 1], {
              extrapolateLeft: "clamp", extrapolateRight: "clamp",
            }),
          }}
        >
          {bars.map((h, i) => {
            const colors = [BRAND.accent, BRAND.accent2, BRAND.accent3, "#F59E0B"];
            const color = colors[i % colors.length];
            return (
              <div
                key={i}
                style={{
                  width: 14,
                  height: `${h}px`,
                  borderRadius: 4,
                  background: `linear-gradient(180deg, ${color} 0%, ${color}66 100%)`,
                  boxShadow: `0 0 10px ${color}44`,
                }}
              />
            );
          })}
        </div>

        {/* Feature Cards */}
        <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: 18 }}>
          {FEATURES.map((feat, i) => {
            const fOpacity = interpolate(
              frame,
              [featStart + i * featStagger, featStart + i * featStagger + 15],
              [0, 1],
              { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
            );
            const fY = interpolate(
              frame,
              [featStart + i * featStagger, featStart + i * featStagger + 20],
              [30, 0],
              { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
            );
            return (
              <div
                key={i}
                style={{
                  background: BRAND.surface,
                  border: `1px solid ${BRAND.border}`,
                  borderRadius: 16,
                  padding: "24px 28px",
                  display: "flex",
                  alignItems: "center",
                  gap: 20,
                  textAlign: "left",
                  opacity: fOpacity,
                  transform: `translateY(${fY}px)`,
                  boxShadow: `0 4px 20px ${feat.color}11`,
                }}
              >
                <div
                  style={{
                    width: 8,
                    height: 50,
                    borderRadius: 4,
                    background: feat.color,
                    flexShrink: 0,
                  }}
                />
                <div>
                  <h3 style={{ fontSize: 26, fontWeight: 700, color: BRAND.text, margin: "0 0 4px" }}>
                    {feat.title}
                  </h3>
                  <p style={{ fontSize: 19, color: BRAND.muted, margin: 0 }}>
                    {feat.desc}
                  </p>
                </div>
              </div>
            );
          })}
        </div>

        {/* Theme pills */}
        <div
          style={{
            display: "flex",
            gap: 12,
            justifyContent: "center",
            marginTop: 28,
            opacity: interpolate(frame, [themeStart, themeStart + 15], [0, 1], {
              extrapolateLeft: "clamp", extrapolateRight: "clamp",
            }),
          }}
        >
          <span style={{ color: BRAND.muted, fontSize: 20, lineHeight: "36px" }}>
            🎨 主题系统:
          </span>
          {THEMES.map((theme, i) => (
            <div
              key={i}
              style={{
                padding: "6px 16px",
                borderRadius: 14,
                background: `${BRAND.surface}`,
                border: `1px solid ${BRAND.border}`,
                color: BRAND.text,
                fontSize: 16,
                fontWeight: 500,
              }}
            >
              {theme}
            </div>
          ))}
          <span style={{ color: BRAND.muted, fontSize: 16, lineHeight: "36px" }}>
            + 10 more
          </span>
        </div>
      </div>
    </AbsoluteFill>
  );
};
