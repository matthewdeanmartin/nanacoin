import { Routes } from '@angular/router';

import { nanaOnly } from './api/guards';
import { IS_DEMO } from './demo/demo';

/**
 * Routes are lazy so that Nana's admin screens - the largest part of the app
 * and the part most people never open - are not in the initial bundle.
 */
export const routes: Routes = [
  { path: 'about', loadComponent: () => import('./pages/about').then(m => m.AboutPage), title: 'About — NanaCoin' },
  { path: 'recipes', loadComponent: () => import('./pages/recipes').then(m => m.RecipesPage), title: 'Lemon bars — NanaCoin' },
  { path: 'ledger', loadComponent: () => import('./pages/public-ledger').then(m => m.PublicLedger), title: 'The notebook — NanaCoin' },
  { path: 'nickles', loadComponent: () => import('./pages/nickles').then(m => m.NicklesPage), title: 'Nana-nickles — NanaCoin' },
  { path: 'redeem', loadComponent: () => import('./pages/nickles').then(m => m.NicklesPage), title: 'Redeem a Nana-nickle — NanaCoin' },
  {
    path: 'market',
    loadComponent: () => import('./pages/market').then((m) => m.MarketPage),
    title: 'Market — NanaCoin',
  },
  {
    path: 'send',
    loadComponent: () => import('./pages/send').then((m) => m.SendPage),
    title: 'Send — NanaCoin',
  },
  {
    path: 'offers',
    loadComponent: () => import('./pages/offers').then((m) => m.OffersPage),
    title: 'Offers — NanaCoin',
  },
  {
    path: 'forex',
    loadComponent: () => import('./pages/forex').then((m) => m.ForexPage),
    title: 'Exchange — NanaCoin',
  },
  {
    path: 'history',
    loadComponent: () => import('./pages/history').then((m) => m.HistoryPage),
    title: 'My Account — NanaCoin',
  },
  {
    path: 'economy',
    loadComponent: () => import('./pages/economy').then((m) => m.EconomyPage),
    title: 'Economy — NanaCoin',
  },
  {
    path: 'clientlog',
    loadComponent: () => import('./pages/clientlog').then((m) => m.ClientLogPage),
    title: 'Browser log — NanaCoin',
  },
  {
    path: 'logs',
    loadComponent: () => import('./pages/logs').then((m) => m.LogsPage),
    title: 'Server logs — NanaCoin',
  },
  {
    path: 'nana',
    loadComponent: () => import('./pages/nana').then((m) => m.NanaPage),
    title: 'Household — NanaCoin',
    // The nav tab is already hidden from everyone else, but hiding a link is
    // not a rule: #/nana typed or bookmarked still rendered the admin screen,
    // with buttons - disable a member, create a user, issue coin - that the
    // server would refuse. The guard makes the hidden tab mean something.
    canActivate: [nanaOnly],
  },
  {
    path: 'invite',
    loadComponent: () => import('./pages/invite').then((m) => m.InvitePage),
    title: 'Invite — NanaCoin',
  },
  {
    path: 'diagnostics',
    loadComponent: () => IS_DEMO
      ? import('./pages/browser-health').then(m => m.BrowserHealth)
      : import('./pages/diagnostics').then((m) => m.DiagnosticsPage),
    title: 'Board Health — NanaCoin',
  },
  { path: '', pathMatch: 'full', redirectTo: 'market' },
  { path: '**', redirectTo: 'market' },
];
