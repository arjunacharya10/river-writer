import { DropdownMenu } from "@kobalte/core/dropdown-menu";
import { Dialog } from "@kobalte/core/dialog";
import { For, Show, createSignal } from "solid-js";
import { call } from "../bridge";
import { Icon } from "./controls";

export function BranchMenu(props: {
  root: string;
  branch: string;
  switchBranch: (name: string) => Promise<void>;
}) {
  const [branches, setBranches] = createSignal<
    { name: string; current: boolean }[]
  >([]);
  const [creating, setCreating] = createSignal(false);
  const [name, setName] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  async function act(job: () => Promise<void>) {
    if (busy()) return;
    setBusy(true);
    setError("");
    try {
      await job();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function load() {
    const root = props.root;
    await act(async () => {
      const list = await call<{ name: string; current: boolean }[]>(
        "git_branches",
        { root },
      );
      if (props.root === root) setBranches(list);
    });
  }
  return (
    <>
      <DropdownMenu
        onOpenChange={(open) => {
          if (open) {
            setBranches([]);
            void load();
          }
        }}
      >
        <DropdownMenu.Trigger
          class="status-branch"
          aria-label={`Branches: ${props.branch}`}
          disabled={busy()}
        >
          <Icon name="git-branch" />
          {props.branch}
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content class="select-content">
            <For each={branches()}>
              {(branch) => (
                <DropdownMenu.Item
                  class="select-item"
                  disabled={branch.current || busy()}
                  onSelect={() =>
                    void act(() => props.switchBranch(branch.name))
                  }
                >
                  {branch.current ? "✓ " : ""}
                  {branch.name}
                </DropdownMenu.Item>
              )}
            </For>
            <DropdownMenu.Item
              class="select-item"
              disabled={busy()}
              onSelect={() => {
                setName("");
                setCreating(true);
              }}
            >
              New branch…
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu>
      <Show when={error()}>
        <span role="alert">{error()}</span>
      </Show>
      <Dialog
        open={creating()}
        onOpenChange={(open) => {
          if (!busy()) setCreating(open);
        }}
      >
        <Dialog.Portal>
          <Dialog.Overlay class="palette-overlay" />
          <Dialog.Content class="palette">
            <Dialog.Title>New branch</Dialog.Title>
            <Dialog.Description>
              Create a branch from the current commit and switch to it.
            </Dialog.Description>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                void act(async () => {
                  const branch = name().trim();
                  if (!branch) return;
                  await call("git_create_branch", {
                    root: props.root,
                    name: branch,
                  });
                  await props.switchBranch(branch);
                  setCreating(false);
                });
              }}
            >
              <input
                autofocus
                aria-label="New branch name"
                value={name()}
                disabled={busy()}
                onInput={(e) => setName(e.currentTarget.value)}
              />
              <Show when={error()}>
                <p role="alert">{error()}</p>
              </Show>
              <button disabled={busy() || !name().trim()}>
                Create and switch
              </button>
              <button
                type="button"
                disabled={busy()}
                onClick={() => setCreating(false)}
              >
                Cancel
              </button>
            </form>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog>
    </>
  );
}
