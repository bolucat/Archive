import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate, spring, useVideoConfig } from "remotion";

// ============ EDITABLE CONSTANTS ============
const BRAND = {
  bg: "#0B0E1A",
  surface: "#141828",
  text: "#F1F5F9",
  accent: "#00E3D5",
  accent2: "#6366F1",
  accent3: "#EC4899",
  muted: "#94A3B8",
  border: "#1E293B",
};

const SECTION_TITLE = "AI 加持的图书库";
const SECTION_SUBTITLE = "EPUB / PDF / MOBI / AZW3 全格式阅读器";

const FORMATS = ["EPUB", "PDF", "TXT", "MOBI", "AZW3", "FB2", "DOCX", "CBZ"];

const AI_FEATURES = [
  { icon: "🔊", title: "AI 语音朗读", desc: "Azure 神经语音 · 跨章节连续朗读", color: BRAND.accent },
  { icon: "🤖", title: "AI 阅读助手", desc: "10+ 大模型 · 本地 RAG 索引 · 多轮对话", color: BRAND.accent2 },
  { icon: "🌍", title: "AI 划词翻译", desc: "DeepL 级品质 · 双语对照 · 整书翻译", color: BRAND.accent3 },
  { icon: "📝", title: "笔记 & 高亮", desc: "自定义高亮色 · 书签 · 批量导出", color: "#F59E0B" },
];
// ============================================

export const Scene5BookLibrary: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const titleOpacity = interpolate(frame, [0, 20], [0, 1], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  // Format badges
  const fmtStart = 20;

  // Feature cards
  const featStart = 60;
  const featStagger = 18;

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
            background: `${BRAND.accent}1A`,
            border: `1px solid ${BRAND.accent}44`,
            color: BRAND.accent,
            fontSize: 20,
            fontWeight: 600,
            marginBottom: 16,
            opacity: titleOpacity,
          }}
        >
          📚 书籍库
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
            margin: "0 0 28px",
          }}
        >
          {SECTION_SUBTITLE}
        </p>

        {/* Format badges */}
        <div
          style={{
            display: "flex",
            gap: 10,
            justifyContent: "center",
            marginBottom: 36,
            flexWrap: "wrap",
          }}
        >
          {FORMATS.map((fmt, i) => {
            const fOpacity = interpolate(
              frame,
              [fmtStart + i * 5, fmtStart + i * 5 + 10],
              [0, 1],
              { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
            );
            const fScale = spring({
              frame: frame - fmtStart - i * 5,
              fps,
              config: { damping: 80, stiffness: 200 },
            });
            return (
              <div
                key={i}
                style={{
                  padding: "6px 18px",
                  borderRadius: 14,
                  background: `${BRAND.surface}`,
                  border: `1px solid ${BRAND.border}`,
                  color: BRAND.muted,
                  fontSize: 18,
                  fontWeight: 600,
                  fontFamily: "monospace",
                  opacity: fOpacity,
                  transform: `scale(${fScale})`,
                }}
              >
                {fmt}
              </div>
            );
          })}
        </div>

        {/* AI Feature Cards */}
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(2, 1fr)",
            gap: 20,
          }}
        >
          {AI_FEATURES.map((feat, i) => {
            const fOpacity = interpolate(
              frame,
              [featStart + i * featStagger, featStart + i * featStagger + 15],
              [0, 1],
              { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
            );
            const fY = interpolate(
              frame,
              [featStart + i * featStagger, featStart + i * featStagger + 20],
              [35, 0],
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
                  borderRadius: 18,
                  padding: "28px 32px",
                  display: "flex",
                  alignItems: "center",
                  gap: 24,
                  textAlign: "left",
                  opacity: fOpacity,
                  transform: `translateY(${fY}px) scale(${fScale})`,
                  boxShadow: `0 6px 24px ${feat.color}11`,
                }}
              >
                <div
                  style={{
                    width: 64,
                    height: 64,
                    borderRadius: 16,
                    background: `${feat.color}1A`,
                    border: `1px solid ${feat.color}44`,
                    display: "flex",
                    justifyContent: "center",
                    alignItems: "center",
                    fontSize: 32,
                    flexShrink: 0,
                  }}
                >
                  {feat.icon}
                </div>
                <div>
                  <h3 style={{ fontSize: 28, fontWeight: 700, color: BRAND.text, margin: "0 0 6px" }}>
                    {feat.title}
                  </h3>
                  <p style={{ fontSize: 20, color: BRAND.muted, margin: 0, lineHeight: 1.4 }}>
                    {feat.desc}
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
