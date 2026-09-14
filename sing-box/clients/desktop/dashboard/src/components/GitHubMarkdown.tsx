import Markdown from "react-markdown";
import remarkGemoji from "remark-gemoji";
import remarkGfm from "remark-gfm";

import styles from "./GitHubMarkdown.module.css";

export function GitHubMarkdown(props: { text: string }) {
  return (
    <div className={styles.markdown}>
      <Markdown
        components={{
          a: ({ node: _node, ref: _ref, ...linkProps }) => (
            <a {...linkProps} target="_blank" rel="noreferrer" />
          ),
        }}
        remarkPlugins={[remarkGfm, remarkGemoji]}
      >
        {props.text}
      </Markdown>
    </div>
  );
}
