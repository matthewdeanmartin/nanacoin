import { type Meta, type StoryObj, moduleMetadata } from "@storybook/angular";
import { SectionTabs } from "../../nanacoin_ui/src/app/ui/section-tabs";

const meta: Meta = {
  title: "Navigation",
  decorators: [moduleMetadata({ imports: [SectionTabs] })],
};
export default meta;

export const PillTabs: StoryObj = {
  name: "Pill tabs",
  render: () => ({
    props: { active: "market" },
    template: `
      <nav class="tabs" aria-label="Main">
        @for (t of ['market', 'send', 'history', 'ledger']; track t) {
          <button type="button" class="tab" [class.tab--active]="active === t"
            [attr.aria-current]="active === t ? 'page' : null"
            (click)="active = t">{{ t }}</button>
        }
      </nav>
    `,
  }),
};

export const SectionTabsComponent: StoryObj = {
  name: "Section tabs (app-section-tabs)",
  render: () => ({
    props: {
      selected: "summary",
      tabs: [
        { id: "summary", label: "Summary" },
        { id: "loans", label: "Loans" },
        { id: "allowances", label: "Allowances" },
      ],
    },
    template: `
      <app-section-tabs label="Account sections" prefix="ds" [tabs]="tabs"
        [(selected)]="selected" />
      @for (t of tabs; track t.id) {
        <section [id]="t.id" role="tabpanel" [attr.aria-labelledby]="'ds-tab-' + t.id"
          [hidden]="selected !== t.id">
          <p>{{ t.label }} panel. Use ← → Home End on the tabs.</p>
        </section>
      }
    `,
  }),
};
