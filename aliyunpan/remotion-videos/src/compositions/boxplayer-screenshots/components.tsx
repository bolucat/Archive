import React from "react";
import {
  Img,
  staticFile,
  useCurrentFrame,
  interpolate,
  spring,
  useVideoConfig,
} from "remotion";

// ============ Brand Colors ============
export const BRAND = {
  bg: "#0B0E1A",
  surface: "#141828",
  surface2: "#1A1F35",
  text: "#F1F5F9",
  accent: "#00E3D5",
  accent2: "#6366F1",
  accent3: "#A855F7",
  muted: "#94A3B8",
  border: "#1E293B",
};

// ============ ScreenshotFrame ============
// Wraps a screenshot in a macOS-style window mockup
export const ScreenshotFrame: React.FC<{
  src: string;
  width: number;
  height?: number;
  title?: string;
  delay?: number;
}> = ({ src, width, height, title, delay = 0 }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const scale = spring({
    frame: frame - delay,
    fps,
    config: { damping: 80, stiffness: 120 },
  });
  const opacity = interpolate(frame, [delay, delay + 15], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const y = interpolate(frame, [delay, delay + 20], [40, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // Remove leading slash for staticFile compatibility
  const cleanSrc = src.startsWith("/") ? src.substring(1) : src;
  const h = height || Math.round(width * 0.625); // default 16:10 aspect

  return (
    <div
      style={{
        width,
        height: h + 36, // title bar height
        opacity,
        transform: `translateY(${y}px) scale(${scale})`,
        borderRadius: 12,
        overflow: "hidden",
        boxShadow: `0 20px 60px rgba(0,0,0,0.5), 0 0 0 1px ${BRAND.border}`,
        background: "#1a1a2e",
      }}
    >
      {/* Title bar */}
      <div
        style={{
          height: 36,
          background: BRAND.surface2,
          display: "flex",
          alignItems: "center",
          paddingLeft: 16,
          gap: 8,
          borderBottom: `1px solid ${BRAND.border}`,
          flexShrink: 0,
        }}
      >
        <div style={{ width: 12, height: 12, borderRadius: "50%", background: "#FF5F57" }} />
        <div style={{ width: 12, height: 12, borderRadius: "50%", background: "#FEBC2E" }} />
        <div style={{ width: 12, height: 12, borderRadius: "50%", background: "#28C840" }} />
        {title && (
          <span
            style={{
              color: BRAND.muted,
              fontSize: 13,
              marginLeft: 12,
              fontWeight: 500,
            }}
          >
            {title}
          </span>
        )}
      </div>
      {/* Screenshot image */}
      <Img
        src={staticFile(cleanSrc)}
        style={{
          width: width,
          height: h,
          objectFit: "cover",
          display: "block",
        }}
      />
    </div>
  );
};

// ============ SectionBadge ============
export const SectionBadge: React.FC<{ icon: string; label: string; color?: string }> = ({
  icon,
  label,
  color = BRAND.accent,
}) => {
  const frame = useCurrentFrame();
  const opacity = interpolate(frame, [0, 15], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const y = interpolate(frame, [0, 15], [-15, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <div
      style={{
        display: "inline-block",
        padding: "6px 20px",
        borderRadius: 16,
        background: `${color}1A`,
        border: `1px solid ${color}44`,
        color,
        fontSize: 20,
        fontWeight: 600,
        opacity,
        transform: `translateY(${y}px)`,
      }}
    >
      {icon} {label}
    </div>
  );
};

// ============ SectionTitle ============
export const SectionTitle: React.FC<{ title: string; subtitle?: string }> = ({ title, subtitle }) => {
  const frame = useCurrentFrame();
  const opacity = interpolate(frame, [5, 25], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const y = interpolate(frame, [5, 25], [-20, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <div style={{ textAlign: "center", marginBottom: 30 }}>
      <h2
        style={{
          fontSize: 52,
          fontWeight: 800,
          color: BRAND.text,
          opacity,
          transform: `translateY(${y}px)`,
          margin: "12px 0 8px",
        }}
      >
        {title}
      </h2>
      {subtitle && (
        <p
          style={{
            fontSize: 26,
            color: BRAND.muted,
            opacity,
            margin: 0,
          }}
        >
          {subtitle}
        </p>
      )}
    </div>
  );
};

// ============ FeaturePills ============
export const FeaturePills: React.FC<{ features: string[]; delay?: number; color?: string }> = ({
  features,
  delay = 0,
  color = BRAND.accent2,
}) => {
  const frame = useCurrentFrame();
  const opacity = interpolate(frame, [delay, delay + 20], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <div
      style={{
        display: "flex",
        gap: 14,
        justifyContent: "center",
        marginTop: 24,
        opacity,
      }}
    >
      {features.map((feat, i) => (
        <div
          key={i}
          style={{
            padding: "8px 22px",
            borderRadius: 20,
            background: `${color}15`,
            border: `1px solid ${color}33`,
            color: BRAND.text,
            fontSize: 20,
            fontWeight: 500,
          }}
        >
          ✓ {feat}
        </div>
      ))}
    </div>
  );
};
