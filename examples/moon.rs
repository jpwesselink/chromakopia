use chromakopia::animate;
use std::time::Duration;

// M    O    O    N
const MOON: &str = "\
#   # ##### ##### #   #
##  ## #   # #   # ##  ##
# ## # #   # #   # # ## #
#    # ##### ##### #    #";

#[tokio::main]
async fn main() {
    // 🌑 = background (first char), lit pixels cycle through the phases
    let anim = animate::petscii("🌑🌒🌓🌔🌕🌖🌗🌘", MOON, 0.3);
    tokio::time::sleep(Duration::from_secs(10)).await;
    anim.stop();
}
