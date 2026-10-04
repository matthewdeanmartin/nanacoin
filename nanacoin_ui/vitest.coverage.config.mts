import { defineConfig } from 'vitest/config';

// Angular supplies compilation, jsdom and test discovery. Only customize coverage output.
export default defineConfig({
  test: { coverage: { provider: 'v8', reportsDirectory: 'coverage/live' } },
});
