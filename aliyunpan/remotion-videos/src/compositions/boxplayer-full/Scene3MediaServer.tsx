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

const SECTION_TITLE = "媒体服务器";
const SECTION_SUBTITLE = "Jellyfin / Emby / Plex 一站接入";

const SERVERS = [
  { name: "Jellyfin", color: "#AA5CC3", desc: "开源媒体服务器" },
  { name: "Emby", color: "#52B54B", desc: "个人媒体中心" },
  { name: "Plex", color: "#E5A00D", desc: "流媒体平台" },
  { name: "WebDAV", color: "#6366F1", desc: "通用协议支持" },
];

const CAPABILITIES = [
  { icon: "🏠", text: "首页聚合：继续观看 / 最近添加 / 电影 / 剧集" },
  { icon: "🗂️", text: "全库浏览：海报墙与列表视图自由切换" },
  { icon: "🔍", text: "跨服务器聚合搜索" },
  { icon: "📺", text: "剧集详情：封面 / 评分 / 分集列表 / 进度记录" },
];
// ============================================

export const Scene3MediaServer: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const titleOpacity = interpolate(frame, [0, 20], [0, 1], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  // Server cards
  const serverStart = 20;
  const serverStagger = 12;

  // Capabilities
  const capStart = 90;
  const capStagger = 18;

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
          🖥️ 媒体服务器
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
            margin: "0 0 40px",
          }}
        >
          {SECTION_SUBTITLE}
        </p>

        <div style={{ display: "flex", gap: 40, alignItems: "flex-start" }}>
          {/* Left: Server Cards */}
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, flex: "0 0 420px" }}>
            {SERVERS.map((server, i) => {
              const sScale = spring({
                frame: frame - serverStart - i * serverStagger,
                fps,
                config: { damping: 80, stiffness: 200 },
              });
              const sOpacity = interpolate(
                frame,
                [serverStart + i * serverStagger, serverStart + i * serverStagger + 12],
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
                    padding: "24px 20px",
                    textAlign: "center",
                    opacity: sOpacity,
                    transform: `scale(${sScale})`,
                    boxShadow: `0 4px 20px ${server.color}11`,
                  }}
                >
                  <div
                    style={{
                      width: 56,
                      height: 56,
                      borderRadius: 14,
                      background: `${server.color}22`,
                      border: `1px solid ${server.color}44`,
                      display: "flex",
                      justifyContent: "center",
                      alignItems: "center",
                      margin: "0 auto 12px",
                    }}
                  >
                    <svg width="30" height="30" viewBox="0 0 24 24" fill="none" stroke={server.color} strokeWidth="2">
                      <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
                      <line x1="8" y1="21" x2="16" y2="21" />
                      <line x1="12" y1="17" x2="12" y2="21" />
                    </svg>
                  </div>
                  <div style={{ fontSize: 24, fontWeight: 700, color: server.color, marginBottom: 4 }}>
                    {server.name}
                  </div>
                  <div style={{ fontSize: 16, color: BRAND.muted }}>
                    {server.desc}
                  </div>
                </div>
              );
            })}
          </div>

          {/* Right: Capabilities */}
          <div style={{ flex: 1, textAlign: "left", paddingTop: 10 }}>
            {CAPABILITIES.map((cap, i) => {
              const cOpacity = interpolate(
                frame,
                [capStart + i * capStagger, capStart + i * capStagger + 15],
                [0, 1],
                { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
              );
              const cX = interpolate(
                frame,
                [capStart + i * capStagger, capStart + i * capStagger + 20],
                [-30, 0],
                { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
              );
              return (
                <div
                  key={i}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 16,
                    marginBottom: 20,
                    opacity: cOpacity,
                    transform: `translateX(${cX}px)`,
                  }}
                >
                  <div
                    style={{
                      width: 48,
                      height: 48,
                      borderRadius: 12,
                      background: `${BRAND.accent}15`,
                      display: "flex",
                      justifyContent: "center",
                      alignItems: "center",
                      fontSize: 24,
                      flexShrink: 0,
                    }}
                  >
                    {cap.icon}
                  </div>
                  <span style={{ color: BRAND.text, fontSize: 22, fontWeight: 500 }}>
                    {cap.text}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </AbsoluteFill>
  );
};
