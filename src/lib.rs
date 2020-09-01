use smol::io::{
    AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt,
    Error, ErrorKind, Result,
};

#[derive(Clone, Debug)]
pub struct Bitmap {
    inner: image::RgbImage,
}

impl Bitmap {
    fn new_from_png(data: Vec<u8>) -> Option<Self> {
        let inner = image::load_from_memory_with_format(
            &data,
            image::ImageFormat::Png,
        )
        .ok()?
        .to_rgb();
        Some(Self { inner })
    }

    pub fn width(&self) -> u32 {
        self.inner.width()
    }

    pub fn height(&self) -> u32 {
        self.inner.height()
    }

    pub fn data(&self) -> &[u8] {
        &self.inner
    }

    pub fn save<Q>(&self, path: Q) -> Result<()>
    where
        Q: std::convert::AsRef<std::path::Path>,
    {
        self.inner
            .save(path)
            .map_err(|e| Error::new(ErrorKind::InvalidInput, e))
    }
}

#[derive(Debug)]
pub struct Scope<T: AsyncRead + AsyncWrite> {
    inner: smol::io::BufReader<T>,
}

impl Scope<smol::net::TcpStream> {
    pub async fn connect<A>(addr: A) -> Result<Self>
    where
        A: smol::net::AsyncToSocketAddrs,
    {
        Ok(Self::new(smol::net::TcpStream::connect(addr).await?))
    }
}

impl<T> Scope<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(inner: T) -> Self {
        Self {
            inner: smol::io::BufReader::new(inner),
        }
    }

    pub async fn write_fmt<'a>(
        &mut self,
        args: std::fmt::Arguments<'a>,
    ) -> Result<()> {
        // horrifying
        let cmdstr: String = format!("{}", args);
        self.inner.get_mut().write_all(cmdstr.as_bytes()).await?;
        Ok(())
    }

    pub async fn read_line(&mut self) -> Result<String> {
        let mut ret = String::new();
        self.inner.read_line(&mut ret).await?;
        Ok(ret)
    }

    pub async fn read_tmc(&mut self) -> Result<Vec<u8>> {
        // tmc header, looks like
        // #NXXXX...
        // with N Xs
        // Xs describe number of bytes in block. ends with a \n, always
        let mut header = [0; 9];
        self.inner.read_exact(&mut header[..2]).await?;
        if header[0] != b'#' {
            return Err(Error::new(ErrorKind::InvalidData, "bad TMC header"));
        }

        let len = header[1] as i16 - b'0' as i16;
        if len < 0 || len > 9 {
            return Err(Error::new(ErrorKind::InvalidData, "bad TMC header"));
        }

        self.inner.read_exact(&mut header[..len as usize]).await?;
        let datalen: usize = std::str::from_utf8(&mut header[..len as usize])
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                Error::new(ErrorKind::InvalidData, "bad TMC header")
            })?;

        let mut data = Vec::with_capacity(datalen);
        (&mut self.inner)
            .take(datalen as u64)
            .read_to_end(&mut data)
            .await?;

        // discard rest until \n
        self.read_line().await?;

        Ok(data)
    }

    pub async fn info(&mut self) -> Result<String> {
        skippy::writeln!(self, :*IDN?).await?;
        self.read_line().await
    }

    pub async fn grab_screen(&mut self) -> Result<Bitmap> {
        skippy::writeln!(self, :DISPlay:DATA?,
                         skippy::Argument::Discrete("ON"),
                         false,
                         skippy::Argument::Discrete("PNG"),
        )
        .await?;

        let data = self.read_tmc().await?;
        Bitmap::new_from_png(data)
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "bad PNG data"))
    }
}
