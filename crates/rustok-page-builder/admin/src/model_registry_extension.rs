impl AdminCanvasController {
    /// Install host-composed provider registries after canonical project decode and immediately
    /// revalidate the document. The controller keeps the mutable Fly registry private; optional
    /// domain modules receive only this narrow installation seam.
    pub fn install_contribution_registries(
        &mut self,
        installer: impl FnOnce(&mut RegistrySet) -> Result<(), String>,
    ) -> Result<(), AdminCanvasError> {
        installer(self.editor.registries_mut()).map_err(AdminCanvasError::Authoring)?;
        let report = self.editor.validate();
        self.synchronize(report);
        Ok(())
    }
}
