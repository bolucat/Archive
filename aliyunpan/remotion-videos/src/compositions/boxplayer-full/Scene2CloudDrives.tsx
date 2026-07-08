import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate, spring, useVideoConfig } from "remotion";

// ============ EDITABLE CONSTANTS ============
const BRAND = {
  bg: "#0B0E1A",
  surface: "#141828",
  text: "#F1F5F9",
  accent: "#00E3D5",
  accent2: "#6366F1",
  muted: "#94A3B8",
  border: "#1E293B",
};

const SECTION_TITLE = "多网盘统一管理";
const SECTION_SUBTITLE = "一个 App 管理你所有的云盘";

const CLOUD_DRIVES = [
  { name: "阿里云盘", color: "#6366F1", short: "Ali" },
  { name: "百度网盘", color: "#3B82F6", short: "Bai" },
  { name: "123网盘", color: "#22D3EE", short: "123" },
  { name: "115网盘", color: "#F59E0B", short: "115" },
  { name: "PikPak", color: "#EC4899", short: "Pi" },
  { name: "Dropbox", color: "#0061FF", short: "Db" },
  { name: "OneDrive", color: "#0078D4", short: "Od" },
  { name: "Box", color: "#0061D5", short: "Bx" },
  { name: "夸克网盘", color: "#8B5CF6", short: "Qk" },
  { name: "天翼云盘", color: "#06B6D4", short: "Ty" },
  { name: "移动云盘", color: "#10B981", short: "Yd" },
  { name: "本地文件夹", color: "#94A3B8", short: "Local" },
];

const FEATURES = [
  "多账号同时登录",
  "文件夹树视图",
  "批量上传下载",
  "海量文件处理",
];
// ============================================

export const Scene2CloudDrives: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  // Title
  const titleOpacity = interpolate(frame, [0, 20], [0, 1], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });
  const titleY = interpolate(frame, [0, 20], [-20, 0], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  // Grid items stagger
  const gridStart = 25;
  const stagger = 7;

  // Feature list
  const featStart = 140;
  const featOpacity = interpolate(frame, [featStart, featStart + 20], [0, 1], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  // Exit
  const exitOpacity = interpolate(frame, [220, 240], [1, 0], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  return (
    <AbsoluteFill
      style={{
        background: `linear-gradient(180deg, ${BRAND.bg} 0%, ${BRAND.surface} 100%)`,
        justifyContent: "center",
        alignItems: "center",
        opacity: exitOpacity,
      }}
    >
      <div style={{ width: 1400, textAlign: "center" }}>
        {/* Section badge */}
        <div
          style={{
            display: "inline-block",
            padding: "6px 20px",
            borderRadius: 16,
            background: `${BRAND.accent}1A`,
            border: `1px solid ${BRAND.accent}44`,
            color: BRAND.accent,
            fontSize: 20,
            fontWeight: 600,
            marginBottom: 16,
            opacity: titleOpacity,
          }}
        >
          ☁ 网盘
        </div>

        <h2
          style={{
            fontSize: 52,
            fontWeight: 800,
            color: BRAND.text,
            opacity: titleOpacity,
            transform: `translateY(${titleY}px)`,
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
            margin: "0 0 40px",
          }}
        >
          {SECTION_SUBTITLE}
        </p>

        {/* Cloud Drive Grid */}
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(4, 1fr)",
            gap: 18,
            marginBottom: 36,
          }}
        >
          {CLOUD_DRIVES.map((drive, i) => {
            const cardScale = spring({
              frame: frame - gridStart - i * stagger,
              fps,
              config: { damping: 80, stiffness: 200 },
            });
            const cardOpacity = interpolate(
              frame,
              [gridStart + i * stagger, gridStart + i * stagger + 12],
              [0, 1],
              { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
            );
            return (
              <div
                key={i}
                style={{
                  background: BRAND.surface,
                  border: `1px solid ${BRAND.border}`,
                  borderRadius: 16,
                  padding: "18px 14px",
                  display: "flex",
                  flexDirection: "column",
                  alignItems: "center",
                  gap: 10,
                  opacity: cardOpacity,
                  transform: `scale(${cardScale})`,
                  boxShadow: `0 4px 20px ${drive.color}11`,
                }}
              >
                <div
                  style={{
                    width: 52,
                    height: 52,
                    borderRadius: 14,
                    background: `${drive.color}22`,
                    display: "flex",
                    justifyContent: "center",
                    alignItems: "center",
                    fontSize: 22,
                    fontWeight: 800,
                    color: drive.color,
                  }}
                >
                  {drive.short}
                </div>
                <span style={{ color: BRAND.text, fontSize: 19, fontWeight: 600 }}>
                  {drive.name}
                </span>
              </div>
            );
          })}
        </div>

        {/* Feature pills */}
        <div
          style={{
            display: "flex",
            gap: 14,
            justifyContent: "center",
            opacity: featOpacity,
          }}
        >
          {FEATURES.map((feat, i) => (
            <div
              key={i}
              style={{
                padding: "8px 22px",
                borderRadius: 20,
                background: `${BRAND.accent2}15`,
                border: `1px solid ${BRAND.accent2}33`,
                color: BRAND.text,
                fontSize: 20,
                fontWeight: 500,
              }}
            >
              ✓ {feat}
            </div>
          ))}
        </div>
      </div>
    </AbsoluteFill>
  );
};
