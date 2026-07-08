import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate, spring, useVideoConfig } from "remotion";

// ============ EDITABLE CONSTANTS ============
const BRAND = {
  bg: "#0B0E1A",
  surface: "#141828",
  text: "#F1F5F9",
  accent: "#00E3D5",
  accent2: "#6366F1",
  accent3: "#F59E0B",
  muted: "#94A3B8",
  border: "#1E293B",
};

const SECTION_TITLE = "智能媒体库";
const SECTION_SUBTITLE = "TMDB 元数据自动刮削，构建你的个人影视库";

// Mock movie posters with colors
const POSTERS = [
  { title: "Inception", year: "2010", color: "#4F46E5", rating: "8.8" },
  { title: "Interstellar", year: "2014", color: "#D97706", rating: "8.6" },
  { title: "Dune", year: "2021", color: "#B45309", rating: "8.0" },
  { title: "Oppenheimer", year: "2023", color: "#7C2D12", rating: "8.4" },
  { title: "The Dark Knight", year: "2008", color: "#1E3A5F", rating: "9.0" },
  { title: "Blade Runner 2049", year: "2017", color: "#991B1B", rating: "8.0" },
];

const SCRAPE_STEPS = [
  { step: "1", text: "扫描网盘 / 本地文件", color: BRAND.accent },
  { step: "2", text: "TMDB 自动识别 & 匹配", color: BRAND.accent2 },
  { step: "3", text: "生成海报墙 + 元数据", color: BRAND.accent3 },
];
// ============================================

export const Scene4MediaLibrary: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const titleOpacity = interpolate(frame, [0, 20], [0, 1], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  // Poster wall stagger
  const posterStart = 25;
  const posterStagger = 8;

  // Scrape steps
  const stepStart = 110;
  const stepStagger = 20;

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
            background: `${BRAND.accent}1A`,
            border: `1px solid ${BRAND.accent}44`,
            color: BRAND.accent,
            fontSize: 20,
            fontWeight: 600,
            marginBottom: 16,
            opacity: titleOpacity,
          }}
        >
          🎬 媒体刮削
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

        <div style={{ display: "flex", gap: 50, alignItems: "center" }}>
          {/* Left: Poster Wall */}
          <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 14, flex: "0 0 480px" }}>
            {POSTERS.map((poster, i) => {
              const pScale = spring({
                frame: frame - posterStart - i * posterStagger,
                fps,
                config: { damping: 80, stiffness: 200 },
              });
              const pOpacity = interpolate(
                frame,
                [posterStart + i * posterStagger, posterStart + i * posterStagger + 12],
                [0, 1],
                { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
              );
              return (
                <div
                  key={i}
                  style={{
                    borderRadius: 12,
                    overflow: "hidden",
                    opacity: pOpacity,
                    transform: `scale(${pScale})`,
                    boxShadow: `0 8px 24px rgba(0,0,0,0.4)`,
                  }}
                >
                  <div
                    style={{
                      width: "100%",
                      height: 160,
                      background: `linear-gradient(135deg, ${poster.color} 0%, ${poster.color}aa 100%)`,
                      padding: "16px 12px",
                      display: "flex",
                      flexDirection: "column",
                      justifyContent: "space-between",
                      textAlign: "left",
                    }}
                  >
                    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
                      <span style={{ color: "rgba(255,255,255,0.6)", fontSize: 14, fontWeight: 600 }}>
                        {poster.year}
                      </span>
                      <span
                        style={{
                          background: "rgba(0,0,0,0.5)",
                          color: "#FBBF24",
                          fontSize: 14,
                          fontWeight: 700,
                          padding: "2px 8px",
                          borderRadius: 8,
                        }}
                      >
                        ★ {poster.rating}
                      </span>
                    </div>
                    <div>
                      <div style={{ color: "white", fontSize: 16, fontWeight: 700, lineHeight: 1.2 }}>
                        {poster.title}
                      </div>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>

          {/* Right: Scrape Steps */}
          <div style={{ flex: 1, textAlign: "left" }}>
            {SCRAPE_STEPS.map((s, i) => {
              const sOpacity = interpolate(
                frame,
                [stepStart + i * stepStagger, stepStart + i * stepStagger + 15],
                [0, 1],
                { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
              );
              const sY = interpolate(
                frame,
                [stepStart + i * stepStagger, stepStart + i * stepStagger + 20],
                [30, 0],
                { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
              );
              // Arrow between steps
              const arrowOpacity = interpolate(
                frame,
                [stepStart + (i + 1) * stepStagger - 5, stepStart + (i + 1) * stepStagger + 5],
                [0, 1],
                { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
              );
              return (
                <React.Fragment key={i}>
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 20,
                      marginBottom: 12,
                      opacity: sOpacity,
                      transform: `translateY(${sY}px)`,
                    }}
                  >
                    <div
                      style={{
                        width: 48,
                        height: 48,
                        borderRadius: "50%",
                        background: `${s.color}22`,
                        border: `2px solid ${s.color}`,
                        display: "flex",
                        justifyContent: "center",
                        alignItems: "center",
                        fontSize: 22,
                        fontWeight: 800,
                        color: s.color,
                        flexShrink: 0,
                      }}
                    >
                      {s.step}
                    </div>
                    <span style={{ color: BRAND.text, fontSize: 24, fontWeight: 600 }}>
                      {s.text}
                    </span>
                  </div>
                  {i < SCRAPE_STEPS.length - 1 && (
                    <div
                      style={{
                        marginLeft: 23,
                        marginBottom: 12,
                        opacity: arrowOpacity,
                        color: BRAND.muted,
                        fontSize: 20,
                      }}
                    >
                      ↓
                    </div>
                  )}
                </React.Fragment>
              );
            })}

            {/* Stats */}
            <div
              style={{
                marginTop: 24,
                display: "flex",
                gap: 24,
                opacity: interpolate(frame, [170, 190], [0, 1], {
                  extrapolateLeft: "clamp", extrapolateRight: "clamp",
                }),
              }}
            >
              {[
                { label: "电影", value: "TMDB" },
                { label: "剧集", value: "TMDB" },
                { label: "动漫", value: "TMDB" },
              ].map((item, i) => (
                <div
                  key={i}
                  style={{
                    padding: "10px 18px",
                    borderRadius: 12,
                    background: `${BRAND.surface}`,
                    border: `1px solid ${BRAND.border}`,
                    textAlign: "center",
                  }}
                >
                  <div style={{ color: BRAND.accent, fontSize: 18, fontWeight: 700 }}>{item.value}</div>
                  <div style={{ color: BRAND.muted, fontSize: 14 }}>{item.label}</div>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </AbsoluteFill>
  );
};
