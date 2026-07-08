import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate, spring, useVideoConfig } from "remotion";

// ============ EDITABLE CONSTANTS ============
const BRAND = {
  bg: "#0B0E1A",
  surface: "#141828",
  text: "#F1F5F9",
  accent: "#6366F1",
  accentCyan: "#22D3EE",
  accentGreen: "#10B981",
  accentOrange: "#F59E0B",
  accentPink: "#EC4899",
  accentPurple: "#A855F7",
  muted: "#94A3B8",
  border: "#1E293B",
};

const FEATURES = [
  {
    icon: "🎬",
    title: "智能媒体库",
    desc: "TMDB 元数据刮削，自动识别电影/剧集/纪录片",
    color: "#6366F1",
    svg: (
      <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18" />
        <line x1="7" y1="2" x2="7" y2="22" />
        <line x1="17" y1="2" x2="17" y2="22" />
        <line x1="2" y1="12" x2="22" y2="12" />
        <line x1="2" y1="7" x2="7" y2="7" />
        <line x1="2" y1="17" x2="7" y2="17" />
        <line x1="17" y1="17" x2="22" y2="17" />
        <line x1="17" y1="7" x2="22" y2="7" />
      </svg>
    ),
  },
  {
    icon: "🎵",
    title: "音乐粒子播放器",
    desc: "实时频谱可视化 · 逐字卡拉OK歌词 · 10段EQ",
    color: "#EC4899",
    svg: (
      <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M9 18V5l12-2v13" />
        <circle cx="6" cy="18" r="3" />
        <circle cx="18" cy="16" r="3" />
      </svg>
    ),
  },
  {
    icon: "⚡",
    title: "高速下载引擎",
    desc: "Aria2c 多线程 · BT 下载 · 远程下载到 NAS/VPS",
    color: "#F59E0B",
    svg: (
      <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
      </svg>
    ),
  },
  {
    icon: "🤖",
    title: "AI 智能搜索",
    desc: "语义搜索 · AI Agent · 跨网盘聚合查找",
    color: "#22D3EE",
    svg: (
      <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M12 2a10 10 0 1 0 10 10A10 10 0 0 0 12 2z" />
        <path d="M8 14s1.5 2 4 2 4-2 4-2" />
        <line x1="9" y1="9" x2="9.01" y2="9" />
        <line x1="15" y1="9" x2="15.01" y2="9" />
      </svg>
    ),
  },
];
// ============================================

export const Scene4Features: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  // Title
  const titleOpacity = interpolate(frame, [0, 20], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // Each feature card timing (staggered)
  const featureDuration = 55; // frames per feature card reveal
  const featureStagger = 50;

  // Exit
  const exitOpacity = interpolate(frame, [250, 270], [1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
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
        {/* Title */}
        <h2
          style={{
            fontSize: 48,
            fontWeight: 700,
            color: BRAND.text,
            opacity: titleOpacity,
            margin: "0 0 50px",
          }}
        >
          四大核心能力，<span style={{ color: BRAND.accentCyan }}>一站搞定</span>
        </h2>

        {/* Feature Cards Grid */}
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(2, 1fr)",
            gap: 24,
          }}
        >
          {FEATURES.map((feature, i) => {
            const startFrame = i * featureStagger;
            const cardOpacity = interpolate(
              frame,
              [startFrame, startFrame + 20],
              [0, 1],
              { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
            );
            const cardY = interpolate(
              frame,
              [startFrame, startFrame + 25],
              [40, 0],
              { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
            );
            const cardScale = spring({
              frame: frame - startFrame,
              fps,
              config: { damping: 80, stiffness: 200 },
            });

            // Icon bounce
            const iconBounce = spring({
              frame: frame - startFrame - 10,
              fps,
              config: { damping: 60, stiffness: 200 },
            });

            return (
              <div
                key={i}
                style={{
                  background: `${BRAND.surface}`,
                  border: `1px solid ${BRAND.border}`,
                  borderRadius: 20,
                  padding: "32px 36px",
                  display: "flex",
                  alignItems: "center",
                  gap: 28,
                  opacity: cardOpacity,
                  transform: `translateY(${cardY}px) scale(${cardScale})`,
                  boxShadow: `0 8px 30px ${feature.color}15`,
                  textAlign: "left",
                }}
              >
                {/* Icon */}
                <div
                  style={{
                    width: 72,
                    height: 72,
                    borderRadius: 18,
                    background: `${feature.color}1A`,
                    border: `1px solid ${feature.color}44`,
                    display: "flex",
                    justifyContent: "center",
                    alignItems: "center",
                    color: feature.color,
                    flexShrink: 0,
                    transform: `scale(${iconBounce})`,
                  }}
                >
                  {feature.svg}
                </div>
                {/* Text */}
                <div>
                  <h3
                    style={{
                      fontSize: 32,
                      fontWeight: 700,
                      color: BRAND.text,
                      margin: "0 0 8px",
                    }}
                  >
                    {feature.title}
                  </h3>
                  <p
                    style={{
                      fontSize: 22,
                      fontWeight: 400,
                      color: BRAND.muted,
                      margin: 0,
                      lineHeight: 1.4,
                    }}
                  >
                    {feature.desc}
                  </p>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </AbsoluteFill>
  );
};
