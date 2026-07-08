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
  accentPurple: "#A855F7",
  muted: "#94A3B8",
  border: "#1E293B",
};

const STATS = [
  { icon: "⭐", value: "GitHub", label: "开源项目", color: "#F59E0B" },
  { icon: "🖥️", value: "3", label: "平台支持", color: "#22D3EE" },
  { icon: "📄", value: "MIT", label: "开源协议", color: "#A855F7" },
];

const PLATFORMS = ["macOS", "Windows", "Linux"];

const GITHUB_URL = "github.com/gaozhangmin/aliyunpan";
const CTA_TEXT = "⭐ Star on GitHub";
// ============================================

export const Scene6CTA: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  // Title
  const titleOpacity = interpolate(frame, [0, 15], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const titleScale = spring({
    frame,
    fps,
    config: { damping: 80, stiffness: 200 },
  });

  // Stats stagger
  const statStart = 15;

  // URL
  const urlOpacity = interpolate(frame, [55, 75], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const urlY = interpolate(frame, [55, 75], [20, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // CTA badge
  const ctaStart = 70;
  const ctaScale = spring({
    frame: frame - ctaStart,
    fps,
    config: { damping: 60, stiffness: 200 },
  });
  const ctaOpacity = interpolate(frame, [ctaStart, ctaStart + 15], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // CTA pulse
  const pulseScale = interpolate(
    Math.sin((frame - ctaStart) * 0.1),
    [-1, 1],
    [1, 1.05]
  );

  // Platform badges
  const platStart = 80;
  const platOpacity = interpolate(frame, [platStart, platStart + 15], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <AbsoluteFill
      style={{
        background: `radial-gradient(circle at 50% 40%, ${BRAND.surface} 0%, ${BRAND.bg} 70%)`,
        justifyContent: "center",
        alignItems: "center",
      }}
    >
      {/* Glow */}
      <div
        style={{
          position: "absolute",
          width: 800,
          height: 800,
          borderRadius: "50%",
          background: `radial-gradient(circle, ${BRAND.accent}15 0%, transparent 70%)`,
        }}
      />

      <div style={{ textAlign: "center", zIndex: 1, width: 1000 }}>
        {/* Title */}
        <h1
          style={{
            fontSize: 56,
            fontWeight: 800,
            color: BRAND.text,
            opacity: titleOpacity,
            transform: `scale(${titleScale})`,
            margin: "0 0 40px",
            textShadow: `0 0 30px ${BRAND.accent}33`,
          }}
        >
          开源 · 免费 · 跨平台
        </h1>

        {/* Stats Row */}
        <div
          style={{
            display: "flex",
            gap: 40,
            justifyContent: "center",
            marginBottom: 50,
          }}
        >
          {STATS.map((stat, i) => {
            const statOpacity = interpolate(
              frame,
              [statStart + i * 12, statStart + i * 12 + 15],
              [0, 1],
              { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
            );
            const statScale = spring({
              frame: frame - statStart - i * 12,
              fps,
              config: { damping: 80, stiffness: 200 },
            });
            return (
              <div
                key={i}
                style={{
                  opacity: statOpacity,
                  transform: `scale(${statScale})`,
                  textAlign: "center",
                }}
              >
                <div
                  style={{
                    fontSize: 48,
                    marginBottom: 8,
                  }}
                >
                  {stat.icon}
                </div>
                <div
                  style={{
                    fontSize: 36,
                    fontWeight: 800,
                    color: stat.color,
                    marginBottom: 4,
                  }}
                >
                  {stat.value}
                </div>
                <div
                  style={{
                    fontSize: 20,
                    color: BRAND.muted,
                    fontWeight: 500,
                  }}
                >
                  {stat.label}
                </div>
              </div>
            );
          })}
        </div>

        {/* GitHub URL */}
        <div
          style={{
            opacity: urlOpacity,
            transform: `translateY(${urlY}px)`,
            marginBottom: 30,
          }}
        >
          <div
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 12,
              padding: "14px 32px",
              borderRadius: 12,
              background: `${BRAND.surface}`,
              border: `1px solid ${BRAND.border}`,
              fontFamily: "monospace",
              fontSize: 28,
              color: BRAND.accentCyan,
            }}
          >
            <svg width="28" height="28" viewBox="0 0 24 24" fill={BRAND.text}>
              <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0 0 24 12c0-6.63-5.37-12-12-12z" />
            </svg>
            {GITHUB_URL}
          </div>
        </div>

        {/* CTA Badge */}
        <div
          style={{
            opacity: ctaOpacity,
            transform: `scale(${ctaScale * pulseScale})`,
            marginBottom: 24,
          }}
        >
          <div
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 10,
              padding: "16px 48px",
              borderRadius: 30,
              background: `linear-gradient(135deg, ${BRAND.accent} 0%, ${BRAND.accentPurple} 100%)`,
              color: "white",
              fontSize: 28,
              fontWeight: 700,
              boxShadow: `0 10px 40px ${BRAND.accent}55`,
            }}
          >
            {CTA_TEXT}
          </div>
        </div>

        {/* Platform badges */}
        <div
          style={{
            display: "flex",
            gap: 16,
            justifyContent: "center",
            opacity: platOpacity,
          }}
        >
          {PLATFORMS.map((p, i) => (
            <div
              key={i}
              style={{
                padding: "6px 20px",
                borderRadius: 16,
                background: `${BRAND.surface}`,
                border: `1px solid ${BRAND.border}`,
                color: BRAND.muted,
                fontSize: 18,
                fontWeight: 600,
              }}
            >
              {p}
            </div>
          ))}
        </div>
      </div>
    </AbsoluteFill>
  );
};
