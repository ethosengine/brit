use gix::bstr::BString;

#[derive(Debug, clap::Args)]
pub struct Platform {
    /// Read from this notes reference.
    #[clap(long = "ref", global = true, value_name = "NOTES_REF", value_parser = crate::shared::AsBString)]
    pub reference: Option<BString>,
    #[clap(subcommand)]
    pub command: Subcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Print the note attached to an object.
    Show {
        /// Object to look up.
        #[clap(default_value = "HEAD", value_parser = crate::shared::AsBString)]
        object: BString,
    },
}
