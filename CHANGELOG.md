# Changelog

## 0.1.0 (2026-07-12)


### Features

* add gpui and open an application window ([af3b938](https://github.com/atqamz/mirufm/commit/af3b938e72af34f874813758a74aa97adc6289a6))
* copy, cut, and paste with an internal clipboard ([ea4c4df](https://github.com/atqamz/mirufm/commit/ea4c4dfd73a972307b3de3d80b7b7f507be811dc))
* **core:** add debounced filesystem watch wrapper over notify ([a995b55](https://github.com/atqamz/mirufm/commit/a995b551d600aa4f9b3006681bf058930df06eb9))
* **core:** add Entry types and cancellable read_dir ([878347b](https://github.com/atqamz/mirufm/commit/878347bce70c2627c6e9a3a0a4c1b6a4e20a8541))
* **core:** add gix-backed per-entry git status with directory roll-up ([9a2169c](https://github.com/atqamz/mirufm/commit/9a2169c6bf7c89b27fc24983e3fa5e57541bbc06))
* **core:** add mkdir and rename operations ([da38045](https://github.com/atqamz/mirufm/commit/da380454e24847dbe9f6fa54e475a6cad615b7de))
* **core:** add move with cross-filesystem fallback ([753f899](https://github.com/atqamz/mirufm/commit/753f8992fdf2f79e883df2fe8c1fbd3600604d74))
* **core:** add navigable column-stack state model ([a13b28c](https://github.com/atqamz/mirufm/commit/a13b28c8cf0dab79931e07f4edb498526ae551f3))
* **core:** add ops error type and collision-free naming ([b74bc35](https://github.com/atqamz/mirufm/commit/b74bc35c31ed9690ab62ec3d5779d307a2db90c1))
* **core:** add priority task scheduler with cancellation ([a6d7953](https://github.com/atqamz/mirufm/commit/a6d7953f01ecc47488c8b6a0f98d9378222cf749))
* **core:** add recursive copy with auto-rename ([16f8fbf](https://github.com/atqamz/mirufm/commit/16f8fbf690775ac8e9ed70d0ce37d9a7e3467355))
* **core:** add sort keys and hidden-file filter ([947ea8e](https://github.com/atqamz/mirufm/commit/947ea8e1b9ef492fcc6edfc3f1ff45139cd5eaab))
* **core:** add trash and permanent delete ([ee12490](https://github.com/atqamz/mirufm/commit/ee124903d158d6d79118e6855ab1e7889cd1f75b))
* **core:** classify a path into a preview model ([96d795e](https://github.com/atqamz/mirufm/commit/96d795eec41e6f6a542e26bdb1513c87e2249fcb))
* **core:** replace single selection with a multi-selection set ([fc795a1](https://github.com/atqamz/mirufm/commit/fc795a13458cddf0857c5550775e1de3e768f296))
* **core:** resolve terminal, desktop apps, and launch argv ([cfb63b7](https://github.com/atqamz/mirufm/commit/cfb63b7c87b16762e04768f87e281ee3122e62c7))
* **core:** store per-repo git status and resolve entry state ([4edd2d0](https://github.com/atqamz/mirufm/commit/4edd2d0b82b3b6be30e86fe084b15a696b3d9b85))
* inline rename and new folder creation ([751bce5](https://github.com/atqamz/mirufm/commit/751bce525abd8dffed57ff23c62fd048740a04de))
* log to state dir and install a panic hook ([e9abd29](https://github.com/atqamz/mirufm/commit/e9abd29388f39880dcaf5b0133250e9a1fff2b25))
* move to trash and permanent delete with confirm ([13c2b13](https://github.com/atqamz/mirufm/commit/13c2b13954aa3dcba50b2e28182c894de2534878))
* multi-select entries with ctrl and shift click ([923aad7](https://github.com/atqamz/mirufm/commit/923aad7bdba4e7e18da7da4f9ab1b82b3135d49a))
* navigate directories across a horizontal column strip ([18f8fee](https://github.com/atqamz/mirufm/commit/18f8feecb587e8139ba7149aa71ae0185a531c84))
* package mirufm as a nix flake output ([608efb1](https://github.com/atqamz/mirufm/commit/608efb1a656d8043850eb34893bdf1c47f3eb79a))
* preview the selected file in a pinned pane ([4e3ed56](https://github.com/atqamz/mirufm/commit/4e3ed56fa3498e53d095ed4e3935ec68220ed267))
* render current directory in a virtualized column ([2a115f3](https://github.com/atqamz/mirufm/commit/2a115f32acd084151967f938ddeea061ad0749a8))
* render per-entry git status badges in columns ([5c84889](https://github.com/atqamz/mirufm/commit/5c84889243d4db44e16f792bc30cfed77bffa15c))
* reuse cached folders and refresh columns on filesystem changes ([ae232be](https://github.com/atqamz/mirufm/commit/ae232be30fe447857c7277b4c2bae307439cb30a))
* right-click menu to open, open-with, and launch a terminal ([97b2100](https://github.com/atqamz/mirufm/commit/97b21008574be5537a40cf6ecda4eb071f5e21ce))


### Bug Fixes

* **app:** stop parking executor threads on blocking channel recv ([4574941](https://github.com/atqamz/mirufm/commit/4574941bb051f9efc47d0869b3381885f3cccb20))
* **core:** guard copy/move into self and reconcile selection across reloads ([ad39387](https://github.com/atqamz/mirufm/commit/ad393872bdabcf55b8653a81a4c5c65934f5773c))
* **core:** keep watching after notify errors, surface read_dir failures ([34d85ac](https://github.com/atqamz/mirufm/commit/34d85ac5e0c270a957ffa1e6a0b41a5013933deb))
* focus the root view for Escape and log spawn failures ([e05de51](https://github.com/atqamz/mirufm/commit/e05de512ab232a9452b3c2e58852080d57f47c3b))
* format symlink test in fs module ([ed3524a](https://github.com/atqamz/mirufm/commit/ed3524a267d67c766dbe76ff822bfc95bc24942d))
* harden clipboard, inline rename, and delete against async reloads ([6580deb](https://github.com/atqamz/mirufm/commit/6580debf649a9a096bc9ef41b7bd4cc7555ce956))
* keep column watchers in sync when navigation is a no-op ([dc831fe](https://github.com/atqamz/mirufm/commit/dc831fe264d209a10146dc7910200bd4664e1dc1))
* keep Open With list matched to the menu that requested it ([befd2c5](https://github.com/atqamz/mirufm/commit/befd2c5960969ae729859513bdc78ac30eb4bac1))
* resolve fix-wave re-review regressions ([ae921c3](https://github.com/atqamz/mirufm/commit/ae921c3acff36de269d228ec4e5888dfdf1fea2e))
* satisfy cloned_ref_to_slice_refs lint in ops tests ([01624f0](https://github.com/atqamz/mirufm/commit/01624f0ad28c8c0cd80cabbc47be1659dc3c4708))
* scroll the preview pane vertically for long content ([c1b8f92](https://github.com/atqamz/mirufm/commit/c1b8f92c4e1cf40fdb3c2189290c7ff822456316))


### Performance Improvements

* **core:** add 100k-entry read_dir benchmark ([84b7cc1](https://github.com/atqamz/mirufm/commit/84b7cc1b0631044d2c22859ff750e96c499acf5a))
