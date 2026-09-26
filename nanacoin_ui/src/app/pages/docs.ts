import { Component, inject, signal } from '@angular/core';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { SectionTabs } from '../ui/section-tabs';

@Component({
  selector:'app-docs', imports:[RouterLink,SectionTabs],
  template:`
    <h1>Docs</h1><p class="lede">A little help for your household bank. Choose a topic.</p>
    <app-section-tabs [tabs]="tabs" [(selected)]="tab" label="Help topics" prefix="docs" />
    <section class="help-page" role="tabpanel" [id]="tab()" [attr.aria-labelledby]="'docs-tab-'+tab()" tabindex="0">
      @switch (tab()) {
        @case ('docs-start') {
          <h2>Start here</h2>
          <p>NanaCoin is a household currency for chores, treats and gifts. You agree what a coin buys. There is no mining, cryptocurrency wallet or promise that coins will gain value.</p>
          <ol><li>Try the demo: choose a person at sign-in. Nana manages the household; the other people use their own accounts.</li>
            <li>Open <a routerLink="/ledger">The Notebook</a> to see payments. Send a gift, buy something, or offer to do a chore.</li>
            <li>Want your own? Choose <button class="help-link" (click)="tab.set('docs-hardware')">Hardware</button> for the tested board and setup guide.</li></ol>
          <p>The demo is pretend money in this browser tab. Reloading starts it over. On your own board, payments are saved there. Your family agrees how to deliver goods and hand over any real cash.</p>
          <p>Inspired by the <a href="https://www.smbc-comics.com/comic/nanacoin">SMBC NanaCoin comic</a>. Use Help → About for the story and project links.</p>
        }
        @case ('docs-account') {
          <h2>My Account</h2>
          <p>Your available coins appear at the top. The flat menu shows one section at a time; clicking another section replaces it, without jumping down a long page.</p>
          <dl><dt>Transactions</dt><dd>Money in and out, newest first. Dollar entries say USD. A correction stays beside its original payment.</dd>
            <dt>TODO</dt><dd>Work and deliveries still owed. Record completion, report a problem, or withdraw a dispute. Marking work done does not move money a second time.</dd>
            <dt>Loans & debts / Lotto</dt><dd>See what you owe, what others owe you, upcoming draws and results.</dd>
            <dt>My offers / My forex bids</dt><dd>Find your outstanding deals and open the relevant page to act on them.</dd></dl>
          <p>Use the account menu beside your name to switch people or sign out. Nana finds My Account and Household inside Accounts.</p>
        }
        @case ('docs-mail') {
          <h2>Mail and payments</h2>
          <dl><dt>Send Money</dt><dd>Choose a person, amount and note. Check the amount before confirming. An empty or zero amount sends a message without moving coins.</dd>
            <dt>Messages</dt><dd>Read and reply to household messages. Avoid putting secrets in payment notes: the Notebook is public.</dd>
            <dt>Allowances</dt><dd>Set a weekly or monthly payment in Send Money. Schedules live in this browser; use Check allowances with the app open to send payments that are due.</dd>
            <dt>Invitations</dt><dd>Draft an invitation for friends or family. A share button opens your chosen social site's posting screen; you decide whether to publish it.</dd>
            <dt>Offers</dt><dd>Review, accept, decline or withdraw a proposed trade. Acceptance moves the agreed coins; posting an offer does not.</dd></dl>
          <p>Sent the wrong amount? Ask the recipient for a refund, or ask Nana to review it in Household. A refund adds a linked entry; it does not erase the original.</p>
        }
        @case ('docs-market') {
          <h2>Buy, sell and exchange</h2>
          <dl><dt>Market / Buy, sell, hire</dt><dd>Advertise an item, offer work, or post something you want to buy. A purchase pays the seller. You still need to arrange delivery or do the work.</dd>
            <dt>Forex</dt><dd>Trade coins for recorded dollars at an agreed price. Read both amounts. The app records the deal; it does not send money through a bank.</dd>
            <dt>Nana-nickles</dt><dd>In the demo, create a one-use voucher, print or share it, and let another person redeem it. Keep the code safe: whoever has it can use it.</dd></dl>
          <p>Gift requests (“cyberbegging”) and digital-art ownership have API support and demo examples in the Notebook. Their own screens are still to come. Art payments cannot be refunded separately from ownership.</p>
        }
        @case ('docs-loans') {
          <h2>Loans and lotto</h2>
          <p><strong>Loans:</strong> a lender offers coins they already own. The borrower reviews and accepts the terms. Interest is simple, not compounded. Payments include principal plus interest; an unpaid amount stays owed. You can repay early.</p>
          <p><strong>Credit at zero:</strong> an accepted credit offer waits until the borrower's balance reaches zero, and then draws once if the lender can fund it.</p>
          <p><strong>Lotto:</strong> Simple pays the pool to one winner. Delayed pays the pool and interest after 30 days. Savings returns everyone's ticket money, with the interest going to one winner. Read the ticket price and dates before buying.</p>
          <p>Nana runs the draws and cannot buy her own tickets. She pays lotto interest from her coins; a shortfall creates new coins. Create draws in Household → Lotto.</p>
        }
        @case ('docs-nana') {
          <h2>Nana is the household treasury</h2>
          <p>Think of Nana as the household's central bank and manager. The current app gives that role one account; it does not yet separate a person from a government, company or club.</p>
          <p>Nana can issue and retire coins, record dollar reserves, trade goods and currencies, buy work, and lend coins she owns. She cannot sell labor or buy tickets in her own lotto. These are the app's current rules, not a claim about what real governments may do.</p>
          <p>For the person who also bakes cookies or does chores, use an ordinary member account for personal activity. Keep Nana's account for the shared treasury. Companies, shared officers and corporate bonds are future features.</p>
          <dl><dt>Household → Ledger</dt><dd>The default view: inspect both sides of recent payments and make permitted corrections, with a reason.</dd>
            <dt>Members</dt><dd>Add a person, replace a forgotten password, or disable access.</dd>
            <dt>Money</dt><dd>Issue coins or record dollars held. Recording dollars does not create real cash. Currency reform changes the unit; preview it before applying.</dd>
            <dt>Lotto / Market desk</dt><dd>Create draws and manage Nana's buy/sell prices and reserve commitments.</dd>
            <dt>Security / Storage</dt><dd>Manage connection trust and saved records on your board. Reset economy removes the household; closing the journal preserves current balances.</dd>
            <dt>Demo data</dt><dd>Add sample activity. On a board these are real saved test entries, so use a test household.</dd></dl>
        }
        @case ('docs-reports') {
          <h2>Reports and troubleshooting</h2>
          <dl><dt>The Notebook</dt><dd>A public record of household transactions. Filter by category or search for a person or description. Original payments remain visible after correction.</dd>
            <dt>Economy</dt><dd>Charts for spending, work, prices and money supply. Tap a label for its meaning. Results depend on the history available.</dd>
            <dt>Nana as Central Bank</dt><dd>Follow reserves, issuance, lending, interest and trading. Some balances are hidden from other accounts. Select the period you want to inspect.</dd>
            <dt>System Info</dt><dd>Board Health checks the connection and device; in the demo it describes your browser. Configuration shows settings. Database and the logs help diagnose problems.</dd>
            <dt>Export Server State</dt><dd>Nana can download a snapshot for inspection or safekeeping. It is not a one-click restore file.</dd></dl>
          <p>If a payment seems stuck, check the Notebook before sending it again. If the board is unreachable, check its power and your Wi-Fi. Keep passwords, voucher codes and private log details out of public help requests.</p>
        }
        @case ('docs-hardware') {
          <h2>Get your own NanaCoin</h2>
          <p><strong>Tested board: ESP32-S3-N16R8 development board, with 16 MB flash and 8 MB PSRAM.</strong> It runs both the household ledger and this website. You need a USB data cable, USB power, a computer for setup and a home Wi-Fi network.</p>
          <p>Board setup currently needs someone comfortable installing computer tools. There is no one-click browser installer. A new blank board needs its initial bootloader and partition layout prepared; the updater below is for an existing Rust NanaCoin board.</p>
          <h3>Update a prepared board</h3>
          <ol><li>Get the <a href="https://github.com/matthewdeanmartin/nanacoin">project files</a>. Complete the <a href="https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/README.md">build prerequisites</a>: Espressif tools, Node dependencies, your private Wi-Fi settings and the board's certificates.</li>
            <li>Connect the correct board with a data cable. Find its USB port; close any serial monitor. In Git Bash, open the <code>nanacoin_rs</code> folder.</li>
            <li>Build and check the plan, then flash. Replace <code>COM9</code> with your board's port:
              <pre>bash scripts/deploy.sh COM9 --dry-run
bash scripts/deploy.sh COM9</pre></li>
            <li>Check it using your board's address:
              <pre>python scripts/probe-board.py --address 192.168.1.158</pre>
              Then open <a href="https://nanacoin.local/">nanacoin.local</a> on the same Wi-Fi. Follow the connection-trust instructions and create your household if prompted.</li></ol>
          <p>The updater checks the board layout and preserves its ledger. If that check fails, stop and use the <a href="https://github.com/matthewdeanmartin/nanacoin/blob/main/nanacoin_rs/DEPLOY.md">full deployment runbook</a>; do not erase the board to get past it. Keep your Wi-Fi settings, certificates and built firmware private.</p>
        }
      }
    </section>`,
  styles:`.help-page { font-size:.95rem; line-height:1.55; } p, dl, ol { margin-block:.65rem; }
    dt { font-weight:650; margin-top:.65rem; } dd { margin:0; } li { margin-block:.5rem; }
    pre { white-space:pre-wrap; overflow-wrap:anywhere; padding:.65rem; background:var(--surface); border:1px solid var(--line); }
    .help-link { color:var(--accent); border:0; padding:0; background:none; font:inherit; text-decoration:underline; cursor:pointer; }`,
})
export class DocsPage {
  readonly tabs=[{id:'docs-start',label:'Start'},{id:'docs-account',label:'My Account'},{id:'docs-mail',label:'Mail'},
    {id:'docs-market',label:'Market'},{id:'docs-loans',label:'Loans & lotto'},{id:'docs-nana',label:'Nana'},
    {id:'docs-reports',label:'Reports'},{id:'docs-hardware',label:'Hardware'}];
  readonly tab=signal('docs-start');
  constructor(){inject(ActivatedRoute).queryParamMap.pipe(takeUntilDestroyed()).subscribe(params=>{
    const tab=params.get('tab');this.tab.set(this.tabs.some(t=>t.id===tab)?tab!:'docs-start');
  });}
}
