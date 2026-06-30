from __future__ import annotations

import tkinter as tk
from pathlib import Path
from tkinter import filedialog, ttk

from .services import ChdConversionOptions, ExtractionOptions, PropertiesSummary, format_size


class PropertiesDialog(tk.Toplevel):
    def __init__(self, master: tk.Misc, title: str, summary: PropertiesSummary) -> None:
        super().__init__(master)
        self.title(title)
        self.geometry("840x420")
        self.transient(master)

        header = ttk.Frame(self, padding=12)
        header.pack(fill="x")

        ttk.Label(
            header,
            text=(
                f"Elementos: {summary.count} | "
                f"Ficheros: {summary.files} | Carpetas: {summary.directories} | "
                f"Tamano total: {format_size(summary.total_size)}"
            ),
        ).pack(anchor="w")

        text = tk.Text(self, wrap="word", height=20)
        text.pack(fill="both", expand=True, padx=12, pady=(0, 12))
        text.insert("1.0", "\n".join(summary.lines))
        text.configure(state="disabled")

        ttk.Button(self, text="Cerrar", command=self.destroy).pack(pady=(0, 12))


class ProgressDialog(tk.Toplevel):
    def __init__(self, master: tk.Misc, title: str) -> None:
        super().__init__(master)
        self.title(title)
        self.geometry("760x360")
        self.transient(master)
        self.protocol("WM_DELETE_WINDOW", self._close_if_done)
        self.done = False

        container = ttk.Frame(self, padding=12)
        container.pack(fill="both", expand=True)

        self.status_var = tk.StringVar(value="Preparando tarea...")
        ttk.Label(container, textvariable=self.status_var).pack(fill="x")

        self.progress = ttk.Progressbar(container, mode="determinate", maximum=100)
        self.progress.pack(fill="x", pady=(8, 8))

        self.log_widget = tk.Text(container, wrap="word", height=14)
        self.log_widget.pack(fill="both", expand=True)

        button_frame = ttk.Frame(container)
        button_frame.pack(fill="x", pady=(10, 0))
        self.close_button = ttk.Button(button_frame, text="Cerrar", command=self.destroy, state="disabled")
        self.close_button.pack(side="right")

    def update_progress(self, ratio: float, message: str) -> None:
        self.status_var.set(message)
        self.progress["value"] = max(0, min(100, ratio * 100))
        self.update_idletasks()

    def append_log(self, message: str) -> None:
        self.log_widget.insert("end", message.rstrip() + "\n")
        self.log_widget.see("end")
        self.update_idletasks()

    def complete(self, message: str) -> None:
        self.done = True
        self.update_progress(1.0, message)
        self.close_button.configure(state="normal")

    def fail(self, message: str) -> None:
        self.done = True
        self.append_log("ERROR: " + message)
        self.status_var.set(message)
        self.close_button.configure(state="normal")

    def _close_if_done(self) -> None:
        if self.done:
            self.destroy()


class ExtractionOptionsDialog(tk.Toplevel):
    def __init__(self, master: tk.Misc, archives: list[Path], preview_callback) -> None:
        super().__init__(master)
        self.title("Descomprimir...")
        self.geometry("620x320")
        self.transient(master)
        self.result: ExtractionOptions | None = None
        self.archives = archives
        self.preview_callback = preview_callback

        self.individual_var = tk.BooleanVar(value=False)
        self.custom_path_var = tk.BooleanVar(value=False)
        self.custom_destination_var = tk.StringVar()
        self.delete_archives_var = tk.BooleanVar(value=False)
        self.overwrite_var = tk.BooleanVar(value=False)

        container = ttk.Frame(self, padding=14)
        container.pack(fill="both", expand=True)

        ttk.Checkbutton(
            container,
            text="En carpeta/as individuales de mismo nombre",
            variable=self.individual_var,
        ).pack(anchor="w", pady=4)

        custom_row = ttk.Frame(container)
        custom_row.pack(fill="x", pady=4)
        ttk.Checkbutton(
            custom_row,
            text="En otra ruta...",
            variable=self.custom_path_var,
            command=self._toggle_custom_path,
        ).pack(side="left")
        self.path_entry = ttk.Entry(custom_row, textvariable=self.custom_destination_var, state="disabled")
        self.path_entry.pack(side="left", fill="x", expand=True, padx=8)
        self.browse_button = ttk.Button(custom_row, text="Seleccionar ruta", command=self._browse, state="disabled")
        self.browse_button.pack(side="right")

        ttk.Checkbutton(
            container,
            text="Eliminar ficheros comprimidos al terminar",
            variable=self.delete_archives_var,
        ).pack(anchor="w", pady=4)

        ttk.Checkbutton(
            container,
            text="Permitir sobreescribir",
            variable=self.overwrite_var,
        ).pack(anchor="w", pady=4)

        button_bar = ttk.Frame(container)
        button_bar.pack(fill="x", side="bottom", pady=(12, 0))
        ttk.Button(button_bar, text="Cancelar", command=self.destroy).pack(side="right")
        ttk.Button(button_bar, text="Aceptar", command=self._accept).pack(side="right", padx=6)
        ttk.Button(button_bar, text="Previsualizar resultado", command=self._preview).pack(side="left")

    def _toggle_custom_path(self) -> None:
        enabled = self.custom_path_var.get()
        state = "normal" if enabled else "disabled"
        self.path_entry.configure(state=state)
        self.browse_button.configure(state=state)

    def _browse(self) -> None:
        selected = filedialog.askdirectory(parent=self, title="Seleccionar destino")
        if selected:
            self.custom_destination_var.set(selected)

    def _build_options(self) -> ExtractionOptions:
        destination_mode = "custom" if self.custom_path_var.get() else "same"
        destination = Path(self.custom_destination_var.get()) if self.custom_destination_var.get().strip() else None
        return ExtractionOptions(
            individual_folders=self.individual_var.get(),
            destination_mode=destination_mode,
            destination_path=destination,
            delete_archives=self.delete_archives_var.get(),
            overwrite=self.overwrite_var.get(),
        )

    def _preview(self) -> None:
        options = self._build_options()
        preview = self.preview_callback(self.archives, options)
        preview_window = tk.Toplevel(self)
        preview_window.title("Previsualizacion")
        preview_window.geometry("900x360")
        preview_window.transient(self)

        wrapper = ttk.Frame(preview_window, padding=12)
        wrapper.pack(fill="both", expand=True)

        columns = ttk.Frame(wrapper)
        columns.pack(fill="both", expand=True)

        left_frame = ttk.LabelFrame(columns, text="Estado actual", padding=8)
        left_frame.pack(side="left", fill="both", expand=True, padx=(0, 8))
        left_text = tk.Text(left_frame, wrap="word")
        left_text.pack(fill="both", expand=True)

        right_frame = ttk.LabelFrame(columns, text="Resultado esperado", padding=8)
        right_frame.pack(side="left", fill="both", expand=True)
        right_text = tk.Text(right_frame, wrap="word")
        right_text.pack(fill="both", expand=True)

        for archive, destination in preview:
            left_text.insert("end", f"{archive}\n")
            right_text.insert("end", f"{destination}\n")

        left_text.configure(state="disabled")
        right_text.configure(state="disabled")

    def _accept(self) -> None:
        self.result = self._build_options()
        self.destroy()


class ChdConversionDialog(tk.Toplevel):
    def __init__(self, master: tk.Misc, folder_mode: bool) -> None:
        super().__init__(master)
        self.title("Convertir a CHD")
        self.geometry("700x320")
        self.transient(master)
        self.result: ChdConversionOptions | None = None

        self.delete_originals_var = tk.BooleanVar(value=False)
        self.name_as_container_var = tk.BooleanVar(value=False)
        self.deposit_to_parent_var = tk.BooleanVar(value=False)
        self.delete_subfolders_var = tk.BooleanVar(value=False)
        self.overwrite_var = tk.BooleanVar(value=False)
        self.folder_mode = folder_mode

        container = ttk.Frame(self, padding=14)
        container.pack(fill="both", expand=True)

        ttk.Checkbutton(
            container,
            text="Eliminar ficheros originales al terminar",
            variable=self.delete_originals_var,
        ).pack(anchor="w", pady=4)

        ttk.Checkbutton(
            container,
            text="Nombrar fichero final como carpeta contenedora",
            variable=self.name_as_container_var,
        ).pack(anchor="w", pady=4)
        ttk.Label(
            container,
            text="Aplicable cuando las ROMs estan dentro de subcarpetas y quieres usar ese nombre para el CHD.",
        ).pack(anchor="w", pady=(0, 8))

        if self.folder_mode:
            ttk.Checkbutton(
                container,
                text="Depositar todo en la carpeta padre",
                variable=self.deposit_to_parent_var,
                command=self._toggle_delete_subfolders,
            ).pack(anchor="w", pady=4)
            ttk.Label(
                container,
                text=(
                    "Solo afecta a ROMs dentro de subcarpetas de la carpeta seleccionada. "
                    "Los CHD finales se guardaran en la carpeta padre elegida."
                ),
            ).pack(anchor="w", pady=(0, 8))

            self.delete_subfolders_check = ttk.Checkbutton(
                container,
                text="Eliminar subcarpetas originales al terminar",
                variable=self.delete_subfolders_var,
                state="disabled",
            )
            self.delete_subfolders_check.pack(anchor="w", pady=4)

        ttk.Checkbutton(
            container,
            text="Permitir sobreescribir",
            variable=self.overwrite_var,
        ).pack(anchor="w", pady=4)

        button_bar = ttk.Frame(container)
        button_bar.pack(fill="x", side="bottom", pady=(12, 0))
        ttk.Button(button_bar, text="Cancelar", command=self.destroy).pack(side="right")
        ttk.Button(button_bar, text="Aceptar", command=self._accept).pack(side="right", padx=6)

    def _toggle_delete_subfolders(self) -> None:
        enabled = self.deposit_to_parent_var.get()
        self.delete_subfolders_check.configure(state="normal" if enabled else "disabled")
        if not enabled:
            self.delete_subfolders_var.set(False)

    def _accept(self) -> None:
        self.result = ChdConversionOptions(
            delete_originals=self.delete_originals_var.get(),
            name_as_container=self.name_as_container_var.get(),
            deposit_to_parent=self.deposit_to_parent_var.get(),
            delete_original_subfolders=self.delete_subfolders_var.get(),
            overwrite=self.overwrite_var.get(),
        )
        self.destroy()
