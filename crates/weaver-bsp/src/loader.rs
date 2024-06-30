use weaver_asset::loading::{LoadAsset, LoadCtx};
use weaver_ecs::prelude::Resource;
use weaver_util::prelude::{anyhow, Result};

use crate::{generator::Bsp, parser::bsp_file};

#[derive(Default, Resource)]
pub struct BspLoader;

impl LoadAsset<Bsp> for BspLoader {
    type Param = ();

    fn load(&self, _param: &mut (), ctx: &mut LoadCtx) -> Result<Bsp> {
        let bytes = ctx.read_original()?;
        let (_, bsp_file) =
            bsp_file(bytes.as_slice()).map_err(|e| anyhow!("Failed to parse bsp file: {:?}", e))?;
        Ok(Bsp::build(&bsp_file))
    }
}
