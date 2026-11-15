# Round-4 probe validation

| Frame / invocation | Result |
|---|---|
| round3-active with `PILL_BBOX=146,12,733,155` | FAIL: bbox supplied; halo R/G 0.85, dropouts 8, pitch 14 crop px, band 77–88 |
| round3-active autodetect | FAIL: body-mask autodetect chooses wrong sparse region |
| round2-active | FAIL: legacy 280px plateau / 0.80 halo regression not retained |

This attempt did not pass validation; further work is needed before relying on the probe.
