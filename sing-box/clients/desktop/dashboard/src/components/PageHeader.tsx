import { createContext, useContext, useMemo, type ReactNode } from "react";
import { createPortal } from "react-dom";

import { useDesktopHost } from "../app/desktop";
import { Icon } from "./Icon";
import { IconButton } from "./ui";

interface ToolbarSlots {
  lead: HTMLElement | null;
  end: HTMLElement | null;
}

const ToolbarSlotsContext = createContext<ToolbarSlots>({ lead: null, end: null });

export function ToolbarSlotsProvider(props: {
  lead: HTMLElement | null;
  end: HTMLElement | null;
  children: ReactNode;
}) {
  const value = useMemo(() => ({ lead: props.lead, end: props.end }), [props.lead, props.end]);
  return <ToolbarSlotsContext.Provider value={value}>{props.children}</ToolbarSlotsContext.Provider>;
}

export function PageHeader(props: {
  title: ReactNode;
  actions?: ReactNode;
  back?: { label: string; onClick: () => void };
}) {
  const host = useDesktopHost();
  const slots = useContext(ToolbarSlotsContext);

  if (host !== null) {
    const back =
      props.back !== undefined ? (
        <IconButton aria-label={props.back.label} onClick={props.back.onClick}>
          <Icon name="arrow_back" size={18} />
        </IconButton>
      ) : null;
    return (
      <>
        {back !== null && slots.lead !== null && createPortal(back, slots.lead)}
        {props.actions !== undefined && slots.end !== null && createPortal(props.actions, slots.end)}
      </>
    );
  }

  return (
    <div className="page-header">
      {props.back !== undefined && (
        <button type="button" className="back-button" aria-label={props.back.label} onClick={props.back.onClick}>
          <Icon name="arrow_back" size={20} />
        </button>
      )}
      <h1 className="page-title">{props.title}</h1>
      {props.actions !== undefined && <div className="actions">{props.actions}</div>}
    </div>
  );
}
