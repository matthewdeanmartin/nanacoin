# The Project

The `hello_wifi` project joins a WiFi network and serves a web page. This page
walks through every file.

## Layout

```text
hello_wifi/
    CMakeLists.txt        project definition
    sdkconfig.defaults    board settings, committed to git
    sdkconfig             generated; holds WiFi credentials; NOT committed
    main/
        CMakeLists.txt    component definition
        Kconfig.projbuild adds entries to the menuconfig UI
        hello_wifi.c      the program
    build/                generated; not committed
```

ESP-IDF is built around **components**. Each is a directory with a
`CMakeLists.txt` that registers it. Your application code lives in a component
conventionally named `main`. The SDK itself is ~120 more of them.

## `CMakeLists.txt`

```cmake
cmake_minimum_required(VERSION 3.16)
include($ENV{IDF_PATH}/tools/cmake/project.cmake)
project(hello_wifi)
```

Three lines, order-sensitive. `project.cmake` pulls in the entire IDF build
system and must be included **before** `project()`.

Note `$ENV{IDF_PATH}` — the build reads it from the environment, which is why an
unactivated shell cannot build.

## `main/CMakeLists.txt`

```cmake
idf_component_register(
    SRCS "hello_wifi.c"
    INCLUDE_DIRS "."
    REQUIRES esp_wifi esp_netif nvs_flash esp_http_server
)
```

`REQUIRES` lists the components this one calls into. Omit one and you get link
errors — undefined references to functions whose headers compiled fine.

## `sdkconfig.defaults` and `sdkconfig`

Two files, and the distinction matters for git.

`sdkconfig.defaults` holds **board settings** and is committed:

```ini
CONFIG_ESP_CONSOLE_USB_CDC=y
CONFIG_ESPTOOLPY_FLASHSIZE_4MB=y
CONFIG_ESP32S2_SPIRAM_SUPPORT=y
```

`CONFIG_ESP_CONSOLE_USB_CDC=y` is the important one for this board: it routes
`printf` and logging over native USB. Without it the monitor stays blank,
because the console would be looking for a UART bridge chip this board does not
have.

`sdkconfig` is **generated** from those defaults plus whatever `menuconfig`
saves — including your WiFi password. It is gitignored. Never commit it.

## `main/Kconfig.projbuild`

Adds entries to the `menuconfig` UI:

```kconfig
menu "Hello WiFi"

    config HELLO_WIFI_SSID
        string "WiFi SSID"
        default "myssid"

    config HELLO_WIFI_PASSWORD
        string "WiFi password"
        default "mypassword"

endmenu
```

Each `config` becomes a `CONFIG_`-prefixed macro available in C:

```c
#define WIFI_SSID      CONFIG_HELLO_WIFI_SSID
#define WIFI_PASSWORD  CONFIG_HELLO_WIFI_PASSWORD
```

The point is keeping credentials out of source control. They live in
`sdkconfig`, which is gitignored, rather than in a `.c` file that is not.

## `main/hello_wifi.c`

### Entry point

```c
void app_main(void)
```

ESP-IDF's equivalent of `main()`. It runs as a FreeRTOS task. **Returning from
it is normal** — other tasks keep running, which is why the web server survives
`app_main` exiting:

```text
I (5756) hello_wifi: HTTP server listening on port 80
I (5756) main_task: Returned from app_main()
```

### NVS first

```c
esp_err_t err = nvs_flash_init();
if (err == ESP_ERR_NVS_NO_FREE_PAGES ||
    err == ESP_ERR_NVS_NEW_VERSION_FOUND) {
    ESP_ERROR_CHECK(nvs_flash_erase());
    err = nvs_flash_init();
}
```

Non-volatile storage. The WiFi driver stores calibration data there and will
not start without it. The erase-and-retry handles a full partition or one
written by a different IDF version.

### Bringing up WiFi

Four steps, in order:

```c
ESP_ERROR_CHECK(esp_netif_init());                 // 1. TCP/IP stack
ESP_ERROR_CHECK(esp_event_loop_create_default());  // 2. event loop
esp_netif_create_default_wifi_sta();               // 3. station interface
esp_wifi_init(&init_cfg);                          // 4. WiFi driver
```

"Station mode" means joining an existing network as a client, as opposed to
"AP mode" where the board *is* the access point.

### Events, not blocking calls

WiFi is asynchronous. `esp_wifi_connect()` returns immediately; the outcome
arrives later as an event. A handler receives them:

```c
static void on_wifi_event(void *arg, esp_event_base_t base,
                          int32_t event_id, void *event_data)
```

This runs on a system task. **Keep it short and never block in it.**

Two events matter:

- `WIFI_EVENT_STA_DISCONNECTED` — failed, or dropped later
- `IP_EVENT_STA_GOT_IP` — associated *and* got a DHCP lease

`GOT_IP` is the real success signal. Associating without a lease leaves you with
no usable network.

### Waiting for a result

FreeRTOS **event groups** let one task sleep until another signals it:

```c
EventBits_t bits = xEventGroupWaitBits(
    s_wifi_events,
    WIFI_CONNECTED_BIT | WIFI_FAILED_BIT,
    pdFALSE, pdFALSE, portMAX_DELAY);
```

`app_main` sleeps here at zero CPU cost while the WiFi task works, and wakes
when the handler sets either bit.

### Diagnosing failures

A bare "disconnected" is ambiguous — wrong password, wrong SSID, and out of
range all look identical. The **reason code** distinguishes them:

```c
wifi_event_sta_disconnected_t *d =
    (wifi_event_sta_disconnected_t *) event_data;
ESP_LOGW(TAG, "disconnected (reason %d); retry %d/%d",
         d->reason, s_retry_count, MAX_RETRIES);
```

| Code | Meaning |
|------|---------|
| 201 `NO_AP_FOUND` | SSID not seen — wrong name, 5GHz-only, or out of range |
| 202 `AUTH_FAIL` | Wrong password |
| 15 `4WAY_HANDSHAKE_TIMEOUT` | Also usually wrong password |
| 200 / 205 | Weak or flaky signal |

### Scanning

`scan_networks()` lists what the board can actually hear, with signal strength.
It answers "is the AP even in range?" directly instead of inferring it from a
failure.

RSSI is in dBm, always negative, closer to zero is stronger:

| Range | Quality |
|-------|---------|
| -30 to -60 | Excellent |
| -60 to -70 | Good |
| -70 to -80 | Marginal |
| below -80 | Unreliable |

!!! warning "Scan ordering"

    The driver refuses to scan while connecting:

    ```text
    W (756) wifi:sta_scan: STA is connecting, scan are not allowed!
    ```

    The obvious structure — handle `WIFI_EVENT_STA_START` by calling
    `esp_wifi_connect()`, then scan — hits exactly this. `STA_START` fires
    during `esp_wifi_start()`, so the connection is already underway before the
    scan runs.

    This project instead scans **before** registering handlers, then calls
    `esp_wifi_connect()` explicitly, leaving the radio idle during the scan.

### The web server

`esp_http_server` maps URIs to handler functions:

```c
httpd_uri_t root = {
    .uri = "/", .method = HTTP_GET, .handler = root_handler,
};
httpd_register_uri_handler(server, &root);
```

Three endpoints:

| Path | Returns |
|------|---------|
| `/` | HTML page with uptime, free heap, IDF version |
| `/health` | `ok` as plain text — handy for `curl` and scripts |
| `/favicon.ico` | `204 No Content` |

The favicon handler exists purely to stop browsers generating a 404 on every
page load:

```text
W (36946) httpd_uri: httpd_uri: URI '/favicon.ico' not found
W (36946) httpd_txrx: httpd_resp_send_err: 404 Not Found
```

Harmless, but it clutters the monitor. `204` is the polite "there isn't one".
