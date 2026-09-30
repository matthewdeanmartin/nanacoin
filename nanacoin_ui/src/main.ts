import { bootstrapApplication } from '@angular/platform-browser';
import { appConfig } from './app/app.config';
import { App } from './app/app';
import { applyStoredTheme } from './app/ui/theme';

applyStoredTheme();

bootstrapApplication(App, appConfig)
  .catch((err) => console.error(err));
