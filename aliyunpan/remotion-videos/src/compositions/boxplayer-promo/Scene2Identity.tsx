import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate, spring, useVideoConfig } from "remotion";

// ============ EDITABLE CONSTANTS ============
const PROJECT = {
  name: "BoxPlayer",
  nameCn: "小白羊网盘",
  tagline: "多网盘统一管理 · 智能媒体库 · 媒体服务器 · 高速下载",
  techStack: ["Electron", "Vue 3", "TypeScript", "MIT License"],
};

const BRAND = {
  bg: "#0B0E1A",
  surface: "#141828",
  text: "#F1F5F9",
  accent: "#6366F1",
  accentCyan: "#22D3EE",
  accentPurple: "#A855F7",
  muted: "#94A3B8",
  border: "#1E293B",
};

const BADGES = [
  { label: "Electron", color: "#47848F" },
  { label: "Vue 3", color: "#42B883" },
  { label: "TypeScript", color: "#3178C6" },
  { label: "MIT", color: "#A855F7" },
];
// ============================================

export const Scene2Identity: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  // Logo icon (box shape) spring in
  const iconScale = spring({
    frame,
    fps,
    config: { damping: 80, stiffness: 200 },
  });

  const iconOpacity = interpolate(frame, [0, 15], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // Name slide up
  const nameY = interpolate(frame, [15, 40], [40, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const nameOpacity = interpolate(frame, [15, 40], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // Tagline
  const tagOpacity = interpolate(frame, [35, 55], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // Badges stagger
  const badgeStart = 50;

  // Exit
  const exitOpacity = interpolate(frame, [75, 90], [1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <AbsoluteFill
      style={{
        background: `linear-gradient(135deg, ${BRAND.bg} 0%, ${BRAND.surface} 100%)`,
        justifyContent: "center",
        alignItems: "center",
        opacity: exitOpacity,
      }}
    >
      {/* Decorative grid lines */}
      <div
        style={{
          position: "absolute",
          inset: 0,
          backgroundImage: `
            linear-gradient(${BRAND.border}08 1px, transparent 1px),
            linear-gradient(90deg, ${BRAND.border}08 1px, transparent 1px)
          `,
          backgroundSize: "60px 60px",
        }}
      />

      <div style={{ textAlign: "center", zIndex: 1 }}>
        {/* Box Icon */}
        <div
          style={{
            width: 120,
            height: 120,
            margin: "0 auto 30px",
            opacity: iconOpacity,
            transform: `scale(${iconScale})`,
            borderRadius: 28,
            background: `linear-gradient(135deg, ${BRAND.accent} 0%, ${BRAND.accentPurple} 100%)`,
            display: "flex",
            justifyContent: "center",
            alignItems: "center",
            boxShadow: `0 20px 60px ${BRAND.accent}44`,
          }}
        >
          <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
            <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
            <line x1="12" y1="22.08" x2="12" y2="12" />
          </svg>
        </div>

        {/* English Name */}
        <h1
          style={{
            fontSize: 72,
            fontWeight: 800,
            color: BRAND.text,
            margin: 0,
            opacity: nameOpacity,
            transform: `translateY(${nameY}px)`,
            letterSpacing: "1px",
          }}
        >
          {PROJECT.name}
        </h1>

        {/* Chinese Name */}
        <h2
          style={{
            fontSize: 36,
            fontWeight: 500,
            color: BRAND.accentCyan,
            margin: "8px 0 20px",
            opacity: nameOpacity,
            transform: `translateY(${nameY}px)`,
          }}
        >
          {PROJECT.nameCn}
        </h2>

        {/* Tagline */}
        <p
          style={{
            fontSize: 28,
            fontWeight: 400,
            color: BRAND.muted,
            margin: "0 0 36px",
            opacity: tagOpacity,
            letterSpacing: "1px",
          }}
        >
          {PROJECT.tagline}
        </p>

        {/* Tech Badges */}
        <div style={{ display: "flex", gap: 16, justifyContent: "center" }}>
          {BADGES.map((badge, i) => {
            const badgeOpacity = interpolate(
              frame,
              [badgeStart + i * 8, badgeStart + i * 8 + 15],
              [0, 1],
              { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
            );
            const badgeY = interpolate(
              frame,
              [badgeStart + i * 8, badgeStart + i * 8 + 15],
              [15, 0],
              { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
            );
            return (
              <div
                key={i}
                style={{
                  padding: "8px 24px",
                  borderRadius: 20,
                  backgroundColor: `${badge.color}22`,
                  border: `1px solid ${badge.color}55`,
                  color: badge.color,
                  fontSize: 20,
                  fontWeight: 600,
                  opacity: badgeOpacity,
                  transform: `translateY(${badgeY}px)`,
                }}
              >
                {badge.label}
              </div>
            );
          })}
        </div>
      </div>
    </AbsoluteFill>
  );
};
