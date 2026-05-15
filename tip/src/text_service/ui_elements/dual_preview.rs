// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 Kan-Ru Chen

use anyhow::Result;
use chewing_tip_core::ipc::{
    client::ChewingIpcClient,
    messages::{HideDualPreview, ShowDualPreview},
    varlink::MethodCall,
};
use exn_anyhow::into_anyhow;

pub(crate) struct DualPreview {
    cth_client: ChewingIpcClient,
}

impl DualPreview {
    pub(crate) fn new(cth_client: ChewingIpcClient) -> Self {
        Self { cth_client }
    }

    pub(crate) fn show(&self, model: &ShowDualPreview) -> Result<()> {
        self.cth_client
            .send(MethodCall {
                method: ShowDualPreview::METHOD.to_string(),
                parameters: serde_json::to_value(model)?,
                oneway: Some(true),
                more: None,
                upgrade: None,
            })
            .map_err(into_anyhow)?;
        Ok(())
    }

    pub(crate) fn hide(&self) -> Result<()> {
        self.cth_client
            .send(MethodCall {
                method: HideDualPreview::METHOD.to_string(),
                parameters: serde_json::to_value(HideDualPreview)?,
                oneway: Some(true),
                more: None,
                upgrade: None,
            })
            .map_err(into_anyhow)?;
        Ok(())
    }
}
