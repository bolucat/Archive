import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate, spring, useVideoConfig } from "remotion";

// ============ EDITABLE CONSTANTS ============
const BRAND = {
  bg: "#0B0E1A",
  surface: "#141828",
  text: "#F1F5F9",
  accent: "#00E3D5",
  accent2: "#F59E0B",
  accent3: "#10B981",
  muted: "#94A3B8",
  border: "#1E293B",
};

const SECTION_TITLE = "高速下载引擎";
const SECTION_SUBTITLE = "Aria2c 多线程 · BT · 远程下载到 NAS / VPS";

const FEATURES = [
  { icon: "⚡", title: "Aria2c 多线程", desc: "主进程托管 · 会话续传 · 崩溃自动重连", color: BRAND.accent2 },
  { icon: "🌱", title: "BT 下载", desc: "UPnP 端口映射 · Tracker 12h 自动同步", color: BRAND.accent3 },
  { icon: "📡", title: "远程下载", desc: "直连 RPC · 下载到远程 VPS / NAS", color: BRAND.accent },
  { icon: "🎯", title: "种子选择器", desc: "BT 任务可只下载选中文件", color: "#6366F1" },
];

const EXTRA_FEATURES = [
  "拖拽添加任务",
  "magnet:// 协议关联",
  "Dock / 任务栏进度环",
  "完成系统通知",
  "批量暂停 / 恢复 / 删除",
  "防休眠管理",
];
// ============================================

export const Scene7Download: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const titleOpacity = interpolate(frame, [0, 20], [0, 1], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  // Progress bar animation
  const progress = interpolate(frame, [20, 120], [0, 78], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });
  const speedText = `${(15 + Math.sin(frame * 0.1) * 3).toFixed(1)} MB/s`;

  // Feature cards
  const featStart = 50;
  const featStagger = 16;

  // Extra features
  const extraStart = 130;

  // Exit
  const exitOpacity = interpolate(frame, [190, 210], [1, 0], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
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
          ⚡ 下载
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

        {/* Download progress bar */}
        <div
          style={{
            background: BRAND.surface,
            border: `1px solid ${BRAND.border}`,
            borderRadius: 14,
            padding: "20px 28px",
            marginBottom: 32,
            opacity: interpolate(frame, [20, 40], [0, 1], {
              extrapolateLeft: "clamp", extrapolateRight: "clamp",
            }),
            textAlign: "left",
          }}
        >
          <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 10 }}>
            <span style={{ color: BRAND.text, fontSize: 20, fontWeight: 600 }}>
              📥 Inception.2010.1080p.BluRay.mkv
            </span>
            <span style={{ color: BRAND.accent2, fontSize: 20, fontWeight: 700, fontFamily: "monospace" }}>
              {speedText}
            </span>
          </div>
          <div
            style={{
              height: 12,
              borderRadius: 6,
              background: `${BRAND.border}`,
              overflow: "hidden",
            }}
          >
            <div
              style={{
                height: "100%",
                width: `${progress}%`,
                borderRadius: 6,
                background: `linear-gradient(90deg, ${BRAND.accent2} 0%, ${BRAND.accent} 100%)`,
                transition: "none",
              }}
            />
          </div>
          <div style={{ display: "flex", justifyContent: "space-between", marginTop: 8 }}>
            <span style={{ color: BRAND.muted, fontSize: 16 }}>
              {progress.toFixed(1)}% · 4.2 GB / 5.4 GB
            </span>
            <span style={{ color: BRAND.muted, fontSize: 16 }}>
              🔗 32 connections · 🌱 8 seeds
            </span>
          </div>
        </div>

        {/* Feature Cards */}
        <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 16, marginBottom: 28 }}>
          {FEATURES.map((feat, i) => {
            const fOpacity = interpolate(
              frame,
              [featStart + i * featStagger, featStart + i * featStagger + 12],
              [0, 1],
              { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
            );
            const fScale = spring({
              frame: frame - featStart - i * featStagger,
              fps,
              config: { damping: 80, stiffness: 200 },
            });
            return (
              <div
                key={i}
                style={{
                  background: BRAND.surface,
                  border: `1px solid ${BRAND.border}`,
                  borderRadius: 16,
                  padding: "24px 20px",
                  textAlign: "center",
                  opacity: fOpacity,
                  transform: `scale(${fScale})`,
                  boxShadow: `0 4px 20px ${feat.color}11`,
                }}
              >
                <div style={{ fontSize: 36, marginBottom: 12 }}>{feat.icon}</div>
                <h3 style={{ fontSize: 22, fontWeight: 700, color: feat.color, margin: "0 0 6px" }}>
                  {feat.title}
                </h3>
                <p style={{ fontSize: 17, color: BRAND.muted, margin: 0, lineHeight: 1.4 }}>
                  {feat.desc}
                </p>
              </div>
            );
          })}
        </div>

        {/* Extra feature pills */}
        <div
          style={{
            display: "flex",
            gap: 10,
            justifyContent: "center",
            flexWrap: "wrap",
            opacity: interpolate(frame, [extraStart, extraStart + 20], [0, 1], {
              extrapolateLeft: "clamp", extrapolateRight: "clamp",
            }),
          }}
        >
          {EXTRA_FEATURES.map((feat, i) => (
            <div
              key={i}
              style={{
                padding: "6px 16px",
                borderRadius: 14,
                background: `${BRAND.surface}`,
                border: `1px solid ${BRAND.border}`,
                color: BRAND.text,
                fontSize: 17,
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
