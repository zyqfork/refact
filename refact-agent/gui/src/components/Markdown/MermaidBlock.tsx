import React, { useEffect, useState, useId, useCallback } from "react";
import { Box, IconButton, Tooltip } from "@radix-ui/themes";
import { CopyIcon, CodeIcon, EyeOpenIcon } from "@radix-ui/react-icons";
import { PreTag } from "./Pre";
import styles from "./Markdown.module.css";
import diagramStyles from "./DiagramBlock.module.css";
import classNames from "classnames";
import { useAppearance } from "../../hooks/useAppearance";

let mermaidInitialized: "dark" | "light" | null = null;

async function getMermaid(theme: "dark" | "light") {
  const mermaid = (await import("mermaid")).default;
  if (mermaidInitialized !== theme) {
    mermaid.initialize({
      startOnLoad: false,
      theme: theme === "dark" ? "dark" : "default",
      securityLevel: "strict",
      fontFamily:
        'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
    });
    mermaidInitialized = theme;
  }
  return mermaid;
}

export type MermaidBlockProps = {
  code: string;
  onCopyClick?: (str: string) => void;
};

const _MermaidBlock: React.FC<MermaidBlockProps> = ({ code, onCopyClick }) => {
  const [svgHtml, setSvgHtml] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showSource, setShowSource] = useState(false);
  const uniqueId = useId().replace(/:/g, "_");
  const { appearance } = useAppearance();
  const theme = appearance === "dark" ? "dark" : "light";

  useEffect(() => {
    let cancelled = false;

    const renderDiagram = async () => {
      try {
        const mermaid = await getMermaid(theme);
        const { svg } = await mermaid.render(
          `mermaid_${uniqueId}`,
          code.trim(),
        );

        if (!cancelled) {
          setSvgHtml(svg);
          setError(null);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
          setSvgHtml(null);
        }
      }
    };

    const timer = setTimeout(() => {
      void renderDiagram();
    }, 100);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [code, uniqueId, theme]);

  const handleToggleSource = useCallback(() => {
    setShowSource((v) => !v);
  }, []);

  const handleCopy = useCallback(() => {
    onCopyClick?.(code);
  }, [onCopyClick, code]);

  if (error) {
    return (
      <Box className={styles.shiki_wrapper}>
        <PreTag className={styles.shiki_pre}>
          <code className={classNames(styles.code, styles.code_block)}>
            {code}
          </code>
        </PreTag>
        <Box className={diagramStyles.error_hint}>
          Mermaid syntax error: {error}
        </Box>
      </Box>
    );
  }

  return (
    <Box className={styles.shiki_wrapper}>
      <Box className={diagramStyles.diagram_container}>
        <Box className={diagramStyles.diagram_toolbar}>
          <Tooltip content={showSource ? "Show diagram" : "Show source"}>
            <IconButton
              size="1"
              variant="ghost"
              onClick={handleToggleSource}
              aria-label={showSource ? "Show diagram" : "Show source"}
            >
              {showSource ? (
                <EyeOpenIcon width={12} height={12} />
              ) : (
                <CodeIcon width={12} height={12} />
              )}
            </IconButton>
          </Tooltip>
          {onCopyClick && (
            <Tooltip content="Copy source">
              <IconButton
                size="1"
                variant="ghost"
                onClick={handleCopy}
                aria-label="Copy mermaid source"
              >
                <CopyIcon width={12} height={12} />
              </IconButton>
            </Tooltip>
          )}
        </Box>
        {showSource ? (
          <PreTag className={styles.shiki_pre}>
            <code className={classNames(styles.code, styles.code_block)}>
              {code}
            </code>
          </PreTag>
        ) : svgHtml ? (
          <Box
            className={diagramStyles.diagram_render}
            dangerouslySetInnerHTML={{ __html: svgHtml }}
          />
        ) : (
          <Box className={diagramStyles.diagram_loading}>Rendering…</Box>
        )}
      </Box>
    </Box>
  );
};

export const MermaidBlock = React.memo(_MermaidBlock);
