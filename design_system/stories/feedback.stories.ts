import {
  Component,
  afterNextRender,
  inject,
  input,
  signal,
} from "@angular/core";
import { type Meta, type StoryObj } from "@storybook/angular";
import { Dialogs, type DialogKind } from "../../nanacoin_ui/src/app/ui/dialog";
import { DialogHost } from "../../nanacoin_ui/src/app/ui/dialog-host";
import { ToastList } from "../../nanacoin_ui/src/app/ui/toast-list";
import { Toasts } from "../../nanacoin_ui/src/app/ui/toasts";

/** Drives the real Toasts service. Seeded toasts stay put for review. */
@Component({
  selector: "ds-toast-demo",
  imports: [ToastList],
  template: `
    <div class="button-row">
      <button
        type="button"
        class="btn"
        (click)="toasts.ok('Sent 5 NC to Timmy.')"
      >
        Show confirmation
      </button>
      <button
        type="button"
        class="btn btn--quiet"
        (click)="toasts.error('Not enough coins for that.')"
      >
        Show error
      </button>
    </div>
    <app-toast-list />
  `,
})
class ToastDemo {
  protected readonly toasts = inject(Toasts);
  readonly seed = input(false);
  constructor() {
    afterNextRender(() => {
      if (!this.seed()) return;
      this.toasts.items.set([
        { id: -1, kind: "ok", text: "Sent 5 NC to Timmy." },
        {
          id: -2,
          kind: "error",
          text: "NanaCoin is busy right now. Try again in a moment.",
        },
      ]);
    });
  }
}

const requests: Record<DialogKind, Parameters<Dialogs["confirm"]>[0]> = {
  confirm: {
    title: "Reverse this payment?",
    message: "Timmy gets the coins back and Nana loses them.",
    detail: ["8 NC · Banana bread", "Sunday, 15:12"],
    confirmLabel: "Reverse",
    danger: true,
  },
  text: { title: "Rename account", initial: "Timmy", required: true },
  password: { title: "New PIN", message: "Four digits or more." },
  number: { title: "How many loaves?", min: 1, max: 12, initial: "1" },
  offer: { title: "Make an offer", message: "For: Old bike" },
};

/** Opens the real DialogHost through the real Dialogs service. */
@Component({
  selector: "ds-dialog-demo",
  imports: [DialogHost],
  template: `
    <div class="button-row">
      @for (k of kinds; track k) {
        <button type="button" class="btn btn--quiet" (click)="open(k)">
          {{ k }}
        </button>
      }
    </div>
    <p class="ds-result">Result: {{ result() }}</p>
    <app-dialog-host />
  `,
})
class DialogDemo {
  private readonly dialogs = inject(Dialogs);
  protected readonly kinds = Object.keys(requests) as DialogKind[];
  protected readonly result = signal("—");
  readonly autoOpen = input<DialogKind | "">("");
  constructor() {
    afterNextRender(() => {
      const k = this.autoOpen();
      if (k) this.open(k);
    });
  }
  protected async open(kind: DialogKind): Promise<void> {
    const req = requests[kind];
    const d = this.dialogs;
    const answer =
      kind === "offer"
        ? await d.offer(req).then((a) => (a ? JSON.stringify(a) : null))
        : kind === "confirm"
          ? await d.confirm(req)
          : kind === "password"
            ? await d.password(req)
            : kind === "number"
              ? await d.number(req)
              : await d.prompt(req);
    this.result.set(answer ?? "cancelled");
  }
}

const meta: Meta = { title: "Feedback" };
export default meta;

export const Toast: StoryObj = {
  render: () => ({
    moduleMetadata: { imports: [ToastDemo] },
    template: `<ds-toast-demo [seed]="true" />`,
  }),
};

type DialogStory = StoryObj<{ autoOpen: DialogKind | "" }>;
const dialogStory = (autoOpen: DialogKind | ""): DialogStory => ({
  args: { autoOpen },
  render: (args) => ({
    props: args,
    moduleMetadata: { imports: [DialogDemo] },
    template: `<ds-dialog-demo [autoOpen]="autoOpen" />`,
  }),
});

export const Dialog = dialogStory("");
export const DialogConfirmDanger = {
  ...dialogStory("confirm"),
  name: "Dialog · confirm (danger)",
};
export const DialogText = { ...dialogStory("text"), name: "Dialog · text" };
export const DialogOffer = { ...dialogStory("offer"), name: "Dialog · offer" };
