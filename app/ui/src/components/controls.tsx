import { Button } from "@kobalte/core/button";
import { Tooltip } from "@kobalte/core/tooltip";
import { Select } from "@kobalte/core/select";
import { Dialog } from "@kobalte/core/dialog";
import Resizable from "@corvu/resizable";
import { For, createEffect, on, untrack, type JSX } from "solid-js";
import { templates, templateNames, type TemplateId } from "../types";
export const Split = Resizable;
/** Keep both panels registered: removing a Corvu 0.2.5 panel corrupts the
 * remaining size on repeated toggles. Visibility changes only the sizes. */
export function SplitVisibility(props: {
  open: boolean;
  panel: 0 | 1;
  defaultSizes: [number, number];
  restoreKey?: number;
}) {
  const context = Resizable.useContext();
  let expanded = props.defaultSizes;
  createEffect(
    on(
      () => [props.open, props.restoreKey] as const,
      ([open, key], previous) =>
        untrack(() => {
          if (key !== previous?.[1]) expanded = props.defaultSizes;
          if (!open && previous?.[0] && key === previous?.[1]) {
            const sizes = context.sizes();
            if (
              sizes.length === 2 &&
              sizes.every((size) => Number.isFinite(size) && size > 0)
            ) {
              const total = sizes[0] + sizes[1];
              expanded = [sizes[0] / total, sizes[1] / total];
            }
          }
          context.setSizes(
            open ? [...expanded] : props.panel === 0 ? [0, 1] : [1, 0],
          );
        }),
    ),
  );
  return null;
}

export function Icon(props: { name: string }) {
  return <i aria-hidden="true" class={`codicon codicon-${props.name}`} />;
}
export function Action(props: {
  label: string;
  icon?: string;
  onClick: () => void;
  active?: boolean;
  disabled?: boolean;
  children?: JSX.Element;
}) {
  return (
    <Tooltip openDelay={500}>
      <Tooltip.Trigger
        as={Button}
        class={`action ${props.active ? "active" : ""}`}
        aria-label={props.label}
        disabled={props.disabled}
        onClick={props.onClick}
      >
        {props.icon && <Icon name={props.icon} />} {props.children}
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content class="tooltip">{props.label}</Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip>
  );
}
export function TemplatePicker(props: {
  value: TemplateId;
  onChange: (v: TemplateId) => void;
}) {
  return (
    <Select<TemplateId>
      options={templates}
      value={props.value}
      onChange={(v) => v && props.onChange(v)}
      itemComponent={(p) => (
        <Select.Item item={p.item} class="select-item">
          <Select.ItemLabel>{templateNames[p.item.rawValue]}</Select.ItemLabel>
          <Select.ItemIndicator>✓</Select.ItemIndicator>
        </Select.Item>
      )}
    >
      <Select.Trigger class="template-picker" aria-label="Page template">
        <Select.Value<TemplateId>>
          {(state) => templateNames[state.selectedOption()!]}
        </Select.Value>
        <Select.Icon>
          <Icon name="chevron-down" />
        </Select.Icon>
      </Select.Trigger>
      <Select.Portal>
        <Select.Content class="select-content">
          <Select.Listbox />
        </Select.Content>
      </Select.Portal>
    </Select>
  );
}
export interface Command {
  id: string;
  label: string;
  key?: string;
  run: () => void;
}
export function Palette(props: {
  open: boolean;
  onClose: () => void;
  query: string;
  setQuery: (v: string) => void;
  items: Command[];
}) {
  return (
    <Dialog open={props.open} onOpenChange={(v) => !v && props.onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay class="palette-overlay" />
        <Dialog.Content class="palette">
          <Dialog.Title class="sr-only">Command Palette</Dialog.Title>
          <Dialog.Description class="sr-only">
            Search commands and choose an action.
          </Dialog.Description>
          <input
            autofocus
            aria-label="Search commands"
            placeholder="Type a command…"
            value={props.query}
            onInput={(e) => props.setQuery(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                props.items[0]?.run();
                props.onClose();
              }
            }}
          />
          <div class="command-list">
            <For each={props.items}>
              {(item) => (
                <Button
                  class="command"
                  onClick={() => {
                    props.onClose();
                    item.run();
                  }}
                >
                  {item.label}
                  <kbd>{item.key}</kbd>
                </Button>
              )}
            </For>
            {!props.items.length && <p>No matching commands</p>}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog>
  );
}
