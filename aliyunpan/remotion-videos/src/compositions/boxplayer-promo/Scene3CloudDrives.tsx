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
  muted: "#94A3B8",
  border: "#1E293B",
};

const CLOUD_DRIVES = [
  { name: "阿里云盘", icon: "☁", color: "#6366F1" },
  { name: "百度网盘", icon: "☁", color: "#3B82F6" },
  { name: "123网盘", icon: "☁", color: "#22D3EE" },
  { name: "115网盘", icon: "☁", color: "#F59E0B" },
  { name: "PikPak", icon: "☁", color: "#EC4899" },
  { name: "Dropbox", icon: "☁", color: "#0061FF" },
  { name: "OneDrive", icon: "☁", color: "#0078D4" },
  { name: "Box", icon: "☁", color: "#0061D5" },
];

const WEBDAV_DRIVES = [
  { name: "夸克网盘", color: "#8B5CF6" },
  { name: "天翼云盘", color: "#06B6D4" },
  { name: "移动云盘", color: "#10B981" },
];

const MEDIA_SERVERS = [
  { name: "Jellyfin", color: "#AA5CC3" },
  { name: "Emby", color: "#52B54B" },
  { name: "Plex", color: "#E5A00D" },
  { name: "WebDAV", color: "#6366F1" },
];
// ============================================

export const Scene3CloudDrives: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  // Section title
  const titleOpacity = interpolate(frame, [0, 20], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  const titleY = interpolate(frame, [0, 20], [-20, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // Cloud drive cards stagger
  const cardStart = 20;
  const cardStagger = 8;

  // WebDAV section
  const webdavStart = 100;
  const webdavOpacity = interpolate(frame, [webdavStart, webdavStart + 20], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // Media server section
  const mediaStart = 130;
  const mediaOpacity = interpolate(frame, [mediaStart, mediaStart + 20], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // Exit
  const exitOpacity = interpolate(frame, [160, 180], [1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
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
        {/* Title */}
        <h2
          style={{
            fontSize: 48,
            fontWeight: 700,
            color: BRAND.text,
            opacity: titleOpacity,
            transform: `translateY(${titleY}px)`,
            margin: "0 0 40px",
          }}
        >
          支持 <span style={{ color: BRAND.accentCyan }}>8+</span> 种网盘与媒体源
        </h2>

        {/* Cloud Drive Grid */}
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(4, 1fr)",
            gap: 20,
            marginBottom: 30,
          }}
        >
          {CLOUD_DRIVES.map((drive, i) => {
            const cardScale = spring({
              frame: frame - cardStart - i * cardStagger,
              fps,
              config: { damping: 80, stiffness: 200 },
            });
            const cardOpacity = interpolate(
              frame,
              [cardStart + i * cardStagger, cardStart + i * cardStagger + 15],
              [0, 1],
              { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
            );
            return (
              <div
                key={i}
                style={{
                  background: `${BRAND.surface}`,
                  border: `1px solid ${BRAND.border}`,
                  borderRadius: 16,
                  padding: "20px 16px",
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
                    width: 48,
                    height: 48,
                    borderRadius: 12,
                    background: `${drive.color}22`,
                    display: "flex",
                    justifyContent: "center",
                    alignItems: "center",
                    fontSize: 28,
                  }}
                >
                  <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke={drive.color} strokeWidth="2">
                    <path d="M17.5 19a4.5 4.5 0 1 0 0-9h-1.8A7 7 0 1 0 4 15.3" />
                    <path d="M4 15.3a4.5 4.5 0 0 0 1.5.7" />
                  </svg>
                </div>
                <span style={{ color: BRAND.text, fontSize: 20, fontWeight: 600 }}>
                  {drive.name}
                </span>
              </div>
            );
          })}
        </div>

        {/* WebDAV + Media Servers Row */}
        <div
          style={{
            display: "flex",
            gap: 16,
            justifyContent: "center",
            marginBottom: 16,
            opacity: webdavOpacity,
          }}
        >
          <span style={{ color: BRAND.muted, fontSize: 22, lineHeight: "40px", fontWeight: 600 }}>
            + WebDAV:
          </span>
          {WEBDAV_DRIVES.map((drive, i) => (
            <div
              key={i}
              style={{
                padding: "8px 20px",
                borderRadius: 20,
                background: `${drive.color}1A`,
                border: `1px solid ${drive.color}44`,
                color: drive.color,
                fontSize: 20,
                fontWeight: 600,
              }}
            >
              {drive.name}
            </div>
          ))}
        </div>

        <div
          style={{
            display: "flex",
            gap: 16,
            justifyContent: "center",
            opacity: mediaOpacity,
          }}
        >
          <span style={{ color: BRAND.muted, fontSize: 22, lineHeight: "40px", fontWeight: 600 }}>
            媒体服务器:
          </span>
          {MEDIA_SERVERS.map((server, i) => (
            <div
              key={i}
              style={{
                padding: "8px 20px",
                borderRadius: 20,
                background: `${server.color}1A`,
                border: `1px solid ${server.color}44`,
                color: server.color,
                fontSize: 20,
                fontWeight: 600,
              }}
            >
              {server.name}
            </div>
          ))}
        </div>
      </div>
    </AbsoluteFill>
  );
};
