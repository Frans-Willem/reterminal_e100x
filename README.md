reTerminal E100x playground
===========================
Purpose
-------
Playing around with Rust, ESP32, Embassy, and eInk.

Goal
----
Stand alone firmware that will:
- Wake-up every 10 minutes (configurable), download a PNG image, and displays it.
- Full color PNG image should be down-converted (dithered) on the device, no pre-processing needed.
- Each wake up all sensors (Battery, temperature, humidity) should be read and reported (Either MQTT, HTTP POST, or as headers of the PNG image get)
- Low power consumption (deep-sleep) between wake-ups
- Buttons should allow to force wake-up and refresh (green) or change between pages (e.g. different URLs)
- WiFi settings and URLs should be configurable through an Access Point captive portal
- Captive portal should be entered on first boot and when refresh button is held for 30 sec.

Stretch goals:
- TRMNL compatibility, allowing switching between TRMNL and other URL using long-press of left/right buttons.

Progress
--------
Currently the device works on the E1002 (color) display only with hard-coded WiFi and URL, refreshes every 10 minutes and on button press.

References
----------
Schematics: (Unsure if they differ apart from the screen)
- E1001: https://files.seeedstudio.com/wiki/reterminal_e10xx/res/202004307_reTerminal_E1001_V1.0_SCH_250805.pdf
- E1002: https://files.seeedstudio.com/wiki/reterminal_e10xx/res/202004321_reTerminal_E1002_V1.0_SCH_250805.pdf

Panels:
- E1001: GooDisplay GDEY075T7 - https://www.good-display.com/product/396.html
- E1002: GooDisplay GDEP073E01 - https://www.good-display.com/blank7.html?productId=533

Other references:
- Rust library for this display having nice names for the commands: https://github.com/xandronak/gdep073e01/blob/main/src/lib.rs
- Another ACEP (but 7 color) display using a similar command set, found out which bit to flip in the PanelSettings to change scan order.
  https://github.com/robertmoro/7ColorEPaperPhotoFrame/blob/main/7ColorEPaperPhotoFrame/epd5in65f.cpp
- Good write-up on some of the technologies used:
  https://hackaday.io/project/179058-understanding-acep-tecnology
