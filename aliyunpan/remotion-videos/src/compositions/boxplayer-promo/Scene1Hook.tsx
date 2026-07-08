import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate, spring, useVideoConfig } from "remotion";

// ============ EDITABLE CONSTANTS ============
const HOOK_TEXT = "你的网盘，一个够吗？";
const HOOK_SUBTEXT = "多网盘散落各处，文件管理混乱，播放体验割裂";

const BRAND = {
  bg: "#0B0E1A",
  bgGradient: "radial-gradient(circle at 50% 50%, #1a1f3a 0%, #0B0E1A 70%)",
  text: "#F1F5F9",
  accent: "#6366F1",
  accentCyan: "#22D3EE",
  muted: "#64748B",
};
// ============================================

export const Scene1Hook: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  // Main text entrance
  const titleScale = spring({
    frame,
    fps,
    config: { damping: 80, stiffness: 200 },
  });

  const titleOpacity = interpolate(frame, [0, 20], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // Subtext fade in
  const subOpacity = interpolate(frame, [40, 70], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const subY = interpolate(frame, [40, 70], [20, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // Exit fade
  const exitOpacity = interpolate(frame, [100, 120], [1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // Pulsing accent circle behind text
  const pulseScale = interpolate(
    Math.sin(frame * 0.05),
    [-1, 1],
    [1, 1.15]
  );

  return (
    <AbsoluteFill
      style={{
        background: BRAND.bgGradient,
        justifyContent: "center",
        alignItems: "center",
        opacity: exitOpacity,
      }}
    >
      {/* Glow circle */}
      <div
        style={{
          position: "absolute",
          width: 600,
          height: 600,
          borderRadius: "50%",
          background: `radial-gradient(circle, ${BRAND.accent}22 0%, transparent 70%)`,
          transform: `scale(${pulseScale})`,
        }}
      />

      <div style={{ textAlign: "center", zIndex: 1 }}>
        <h1
          style={{
            fontSize: 80,
            fontWeight: 800,
            color: BRAND.text,
            opacity: titleOpacity,
            transform: `scale(${titleScale})`,
            margin: 0,
            letterSpacing: "2px",
            textShadow: `0 0 40px ${BRAND.accent}44`,
          }}
        >
          {HOOK_TEXT}
        </h1>
        <p
          style={{
            fontSize: 32,
            fontWeight: 400,
            color: BRAND.muted,
            opacity: subOpacity,
            transform: `translateY(${subY}px)`,
            marginTop: 30,
            letterSpacing: "1px",
          }}
        >
          {HOOK_SUBTEXT}
        </p>
      </div>
    </AbsoluteFill>
  );
};
