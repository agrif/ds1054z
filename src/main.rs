fn main() -> smol::io::Result<()> {
    smol::block_on(async {
        let args: Vec<String> = std::env::args().collect();
        let mut scope = ds1054z::Scope::connect(&args[1]).await?;
        let bmp = scope.grab_screen().await?;
        bmp.save("grab.png")?;
        Ok(())
    })
}
