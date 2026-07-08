import React from "react";
import { AbsoluteFill, useCurrentFrame, interpolate, spring, useVideoConfig } from "remotion";

// ============ EDITABLE CONSTANTS ============
const TERMINAL = {
  bg: "#1E1E2E",
  text: "#CDD6F4",
  prompt: "#89B4FA",
  success: "#A6E3A1",
  comment: "#6C7086",
  cursorColor: "#F5E0DC",
  border: "#313244",
};

const BRAND = {
  bg: "#0B0E1A",
  surface: "#141828",
  text: "#F1F5F9",
  accent: "#00E3D5",
  accent2: "#6366F1",
  muted: "#94A3B8",
};

const SECTION_TITLE = "clouddrive-cli";
const SECTION_SUBTITLE = "面向终端和 AI Agent 的自动化入口";

const COMMANDS = [
  { cmd: "npm install -g clouddrive-cli", output: "✓ 安装完成" },
  { cmd: 'clouddrive-cli files search --name "Inception"', output: "✓ 找到 3 个结果" },
  { cmd: "clouddrive-cli organize analyze --provider aliyun", output: "✓ 分析完成" },
];

const AGENT_TAGS = ["Claude Desktop", "Cursor", "MCP Server", "Windsurf", "Claude Code"];
// ============================================

export const Scene8CLI: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const titleOpacity = interpolate(frame, [0, 15], [0, 1], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  // Terminal slide up
  const termY = interpolate(frame, [10, 35], [50, 0], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });
  const termOpacity = interpolate(frame, [10, 35], [0, 1], {
    extrapolateLeft: "clamp", extrapolateRight: "clamp",
  });

  // Command typing
  const cmdStart = 30;
  const cmdGap = 26;

  // Agent tags
  const tagStart = 120;

  // Exit
  const exitOpacity = interpolate(frame, [165, 180], [1, 0], {
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
      <div style={{ width: 1100, textAlign: "center" }}>
        {/* Badge */}
        <div
          style={{
            display: "inline-block",
            padding: "6px 20px",
            borderRadius: 16,
            background: `${BRAND.accent2}1A`,
            border: `1px solid ${BRAND.accent2}44`,
            color: BRAND.accent,
            fontSize: 20,
            fontWeight: 600,
            marginBottom: 16,
            opacity: titleOpacity,
            fontFamily: "monospace",
          }}
        >
          🖥️ CLI 插件
        </div>

        <h2
          style={{
            fontSize: 44,
            fontWeight: 800,
            color: BRAND.accent,
            opacity: titleOpacity,
            margin: "0 0 8px",
            fontFamily: "monospace",
          }}
        >
          {SECTION_TITLE}
        </h2>
        <p
          style={{
            fontSize: 24,
            color: BRAND.muted,
            opacity: titleOpacity,
            margin: "0 0 32px",
          }}
        >
          {SECTION_SUBTITLE}
        </p>

        {/* Terminal */}
        <div
          style={{
            background: TERMINAL.bg,
            borderRadius: 16,
            border: `1px solid ${TERMINAL.border}`,
            padding: 28,
            textAlign: "left",
            opacity: termOpacity,
            transform: `translateY(${termY}px)`,
            boxShadow: "0 25px 50px rgba(0,0,0,0.5)",
            fontFamily: "monospace",
          }}
        >
          {/* Window dots */}
          <div style={{ display: "flex", gap: 8, marginBottom: 20 }}>
            <div style={{ width: 12, height: 12, borderRadius: "50%", background: "#FF5F57" }} />
            <div style={{ width: 12, height: 12, borderRadius: "50%", background: "#FEBC2E" }} />
            <div style={{ width: 12, height: 12, borderRadius: "50%", background: "#28C840" }} />
          </div>

          {COMMANDS.map((cmd, i) => {
            const startFrame = cmdStart + i * cmdGap;
            const localFrame = frame - startFrame;
            const charsToShow = Math.min(Math.max(Math.floor(localFrame / 1.5), 0), cmd.cmd.length);
            const visibleCmd = cmd.cmd.slice(0, charsToShow);
            const cmdDone = charsToShow >= cmd.cmd.length;
            const outputDelay = 6;
            const outputOpacity = interpolate(
              frame,
              [startFrame + cmd.cmd.length * 1.5 + outputDelay, startFrame + cmd.cmd.length * 1.5 + outputDelay + 8],
              [0, 1],
              { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
            );
            const showCursor = !cmdDone && Math.sin(localFrame * 0.3) > 0;

            return (
              <div key={i} style={{ marginBottom: 14 }}>
                <div style={{ fontSize: 21 }}>
                  <span style={{ color: TERMINAL.prompt }}>$ </span>
                  <span style={{ color: TERMINAL.text }}>{visibleCmd}</span>
                  {showCursor && <span style={{ color: TERMINAL.cursorColor }}>█</span>}
                </div>
                {cmdDone && (
                  <div style={{ color: TERMINAL.success, fontSize: 19, marginTop: 4, opacity: outputOpacity }}>
                    {cmd.output}
                  </div>
                )}
              </div>
            );
          })}
        </div>

        {/* Agent tags */}
        <div
          style={{
            display: "flex",
            gap: 10,
            justifyContent: "center",
            marginTop: 24,
            opacity: interpolate(frame, [tagStart, tagStart + 15], [0, 1], {
              extrapolateLeft: "clamp", extrapolateRight: "clamp",
            }),
          }}
        >
          <span style={{ color: BRAND.muted, fontSize: 20, lineHeight: "34px" }}>兼容:</span>
          {AGENT_TAGS.map((tag, i) => (
            <div
              key={i}
              style={{
                padding: "5px 16px",
                borderRadius: 14,
                background: `${BRAND.accent2}1A`,
                border: `1px solid ${BRAND.accent2}44`,
                color: BRAND.accent,
                fontSize: 17,
                fontWeight: 600,
              }}
            >
              {tag}
            </div>
          ))}
        </div>
      </div>
    </AbsoluteFill>
  );
};
