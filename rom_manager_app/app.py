from __future__ import annotations

import queue
import threading
import tkinter as tk
from pathlib import Path
from tkinter import filedialog, messagebox, simpledialog, ttk

from .dialogs import ChdConversionDialog, ExtractionOptionsDialog, ProgressDialog, PropertiesDialog
from .paths import get_project_root
from .services import (
    ARCHIVE_EXTENSIONS,
    CD_SOURCE_EXTENSIONS,
    ClipboardPayload,
    ChdConversionOptions,
    ExtractionOptions,
    MAME_CHDMAN_DOCS_URL,
    MAME_GITHUB_URL,
    MAME_RELEASE_URL,
    build_entry,
    build_extraction_preview,
    copy_or_move_paths,
    create_file,
    create_folder,
    detect_chdman,
    extract_archives,
    format_extension_labels,
    format_size,
    gather_chd_sources_from_selection,
    is_archive,
    iter_search,
    list_directory,
    load_chdman_metadata,
    open_with_dialog,
    open_with_system,
    remove_paths,
    rename_path,
    run_chd_conversion,
    summarize_paths,
)


class RomManagerApp(tk.Tk):
    def __init__(self) -> None:
        super().__init__()
        self.title("scriptsManageRoms")
        self.geometry("1320x780")
        self.minsize(980, 640)

        self.project_root = get_project_root()
        self.root_path: Path | None = None
        self.current_dir: Path | None = None
        self.current_entries = []
        self.clipboard_payload: ClipboardPayload | None = None
        self.tree_path_by_id: dict[str, Path] = {}
        self.content_path_by_id: dict[str, Path] = {}
        self.loaded_tree_dirs: set[Path] = set()
        self.pending_content_token = 0
        self.pending_search_token = 0
        self.global_search_after_id: str | None = None
        self.local_search_after_id: str | None = None
        self.local_search_window: tk.Toplevel | None = None
        self.local_search_var = tk.StringVar(value="")
        self.local_recursive_var = tk.BooleanVar(value=False)
        self.type_select_buffer = ""
        self.type_select_after_id: str | None = None
        self.activity_count = 0

        self.path_var = tk.StringVar(value="Selecciona una carpeta raiz para comenzar.")
        self.global_search_var = tk.StringVar(value="")
        self.info_var = tk.StringVar(value="Sin seleccion.")

        self._configure_style()
        self._create_icons()
        self._build_menu()
        self._build_layout()
        self._bind_shortcuts()

    def _configure_style(self) -> None:
        style = ttk.Style(self)
        try:
            style.theme_use("clam")
        except tk.TclError:
            pass
        style.configure("Treeview", rowheight=24)

    def _create_icons(self) -> None:
        self.folder_icon = tk.PhotoImage(width=14, height=14)
        self.folder_icon.put("#cf9f34", to=(1, 4, 13, 12))
        self.folder_icon.put("#ebc86a", to=(1, 2, 7, 5))

        self.file_icon = tk.PhotoImage(width=14, height=14)
        self.file_icon.put("#d9dfe5", to=(2, 1, 11, 12))
        self.file_icon.put("#8294a7", to=(2, 1, 11, 2))

        self.multi_icon = tk.PhotoImage(width=14, height=14)
        self.multi_icon.put("#9cb3c7", to=(1, 2, 10, 11))
        self.multi_icon.put("#6a8299", to=(4, 4, 13, 13))

    def _build_menu(self) -> None:
        menu_bar = tk.Menu(self)

        file_menu = tk.Menu(menu_bar, tearoff=False)
        file_menu.add_command(label="Abrir carpeta raiz...", command=self.choose_root_directory)
        file_menu.add_command(label="Crear carpeta", command=self.create_folder_action)
        file_menu.add_command(label="Crear fichero", command=self.create_file_action)
        file_menu.add_command(label="Pegar", command=self.paste_action)
        file_menu.add_separator()
        file_menu.add_command(label="Recargar", command=self.reload_current_view)
        file_menu.add_separator()
        file_menu.add_command(label="Salir", command=self.destroy)
        menu_bar.add_cascade(label="File", menu=file_menu)

        tools_menu = tk.Menu(menu_bar, tearoff=False)
        tools_menu.add_command(label="Buscar en carpeta actual", command=self.open_local_search)
        tools_menu.add_command(label="Limpiar busqueda global", command=self.clear_global_search)
        tools_menu.add_command(label="Informacion de herramientas", command=self.show_tools_info)
        menu_bar.add_cascade(label="Tools", menu=tools_menu)

        about_menu = tk.Menu(menu_bar, tearoff=False)
        about_menu.add_command(label="Acerca de", command=self.show_about)
        menu_bar.add_cascade(label="About", menu=about_menu)

        self.config(menu=menu_bar)

    def _build_layout(self) -> None:
        container = ttk.Frame(self, padding=10)
        container.pack(fill="both", expand=True)

        top_bar = ttk.Frame(container)
        top_bar.pack(fill="x")

        ttk.Button(top_bar, text="Abrir carpeta raiz", command=self.choose_root_directory).pack(side="left")
        ttk.Label(top_bar, text="Ruta actual").pack(side="left", padx=(12, 6))
        ttk.Entry(top_bar, textvariable=self.path_var).pack(side="left", fill="x", expand=True)
        ttk.Label(top_bar, text="Buscar").pack(side="left", padx=(12, 6))
        search_entry = ttk.Entry(top_bar, textvariable=self.global_search_var, width=30)
        search_entry.pack(side="left")
        search_entry.bind("<KeyRelease>", self._schedule_global_search)
        ttk.Button(top_bar, text="Limpiar", command=self.clear_global_search).pack(side="left", padx=(6, 0))

        pane = ttk.Panedwindow(container, orient="horizontal")
        pane.pack(fill="both", expand=True, pady=(10, 6))

        left_frame = ttk.Frame(pane, padding=(0, 0, 8, 0))
        right_frame = ttk.Frame(pane)
        pane.add(left_frame, weight=1)
        pane.add(right_frame, weight=3)

        self.tree = ttk.Treeview(left_frame, show="tree")
        self.tree.pack(side="left", fill="both", expand=True)
        tree_scroll = ttk.Scrollbar(left_frame, orient="vertical", command=self.tree.yview)
        tree_scroll.pack(side="right", fill="y")
        self.tree.configure(yscrollcommand=tree_scroll.set)
        self.tree.bind("<<TreeviewOpen>>", self._on_tree_open)
        self.tree.bind("<<TreeviewSelect>>", self._on_tree_select)
        self.tree.bind("<Button-3>", self._on_tree_right_click)
        self.tree.bind("<Double-1>", self._on_tree_double_click)

        self.content_tree = ttk.Treeview(
            right_frame,
            columns=("type", "size", "modified", "location"),
            show="tree headings",
            selectmode="extended",
        )
        self.content_tree.heading("#0", text="Nombre")
        self.content_tree.heading("type", text="Tipo")
        self.content_tree.heading("size", text="Tamano")
        self.content_tree.heading("modified", text="Modificado")
        self.content_tree.heading("location", text="Ubicacion")
        self.content_tree.column("#0", width=280, anchor="w")
        self.content_tree.column("type", width=120, anchor="w")
        self.content_tree.column("size", width=110, anchor="e")
        self.content_tree.column("modified", width=155, anchor="center")
        self.content_tree.column("location", width=380, anchor="w")
        self.content_tree.pack(side="left", fill="both", expand=True)
        content_scroll = ttk.Scrollbar(right_frame, orient="vertical", command=self.content_tree.yview)
        content_scroll.pack(side="right", fill="y")
        self.content_tree.configure(yscrollcommand=content_scroll.set)
        self.content_tree.bind("<<TreeviewSelect>>", self._on_content_select)
        self.content_tree.bind("<Double-1>", self._on_content_double_click)
        self.content_tree.bind("<Button-3>", self._on_content_right_click)
        self.content_tree.bind("<KeyPress>", self._on_content_keypress)

        bottom_bar = ttk.Frame(container)
        bottom_bar.pack(fill="x")
        self.info_icon_label = ttk.Label(bottom_bar, image=self.file_icon)
        self.info_icon_label.pack(side="left")
        ttk.Label(bottom_bar, textvariable=self.info_var).pack(side="left", padx=(8, 0), fill="x", expand=True)

        self.activity_bar = ttk.Progressbar(container, mode="indeterminate")
        self.activity_bar.pack(fill="x", pady=(6, 0))

    def _bind_shortcuts(self) -> None:
        self.bind("<Control-f>", lambda _: self.open_local_search())
        self.bind("<F5>", lambda _: self.reload_current_view())

    def choose_root_directory(self) -> None:
        selected = filedialog.askdirectory(parent=self, title="Selecciona la carpeta de ROMs")
        if selected:
            self.load_root_directory(Path(selected))

    def load_root_directory(self, root: Path) -> None:
        self.root_path = root
        self.current_dir = root
        self.path_var.set(str(root))
        self.global_search_var.set("")
        self.clear_local_search_state(restore=False)

        self.tree.delete(*self.tree.get_children())
        self.tree_path_by_id.clear()
        self.loaded_tree_dirs.clear()

        root_id = self.tree.insert("", "end", text=root.name or str(root), image=self.folder_icon, open=True)
        self.tree_path_by_id[root_id] = root
        self.tree.insert(root_id, "end", text="__placeholder__")
        self._populate_tree_children(root_id, root)
        self._select_tree_item(root)
        self.load_directory_async(root)

    def _populate_tree_children(self, item_id: str, path: Path) -> None:
        if path in self.loaded_tree_dirs:
            return
        self.tree.delete(*self.tree.get_children(item_id))
        try:
            entries = list_directory(path)
        except RuntimeError:
            return
        for entry in entries:
            child_id = self.tree.insert(
                item_id,
                "end",
                text=entry.name,
                image=self.folder_icon if entry.is_dir else self.file_icon,
                open=False,
            )
            self.tree_path_by_id[child_id] = entry.path
            if entry.is_dir:
                self.tree.insert(child_id, "end", text="__placeholder__")
        self.loaded_tree_dirs.add(path)

    def _on_tree_open(self, _event=None) -> None:
        selected = self.tree.selection()
        if not selected:
            return
        path = self.tree_path_by_id.get(selected[0])
        if path and path.is_dir():
            self._populate_tree_children(selected[0], path)

    def _on_tree_select(self, _event=None) -> None:
        selected = self.tree.selection()
        if not selected:
            return
        path = self.tree_path_by_id.get(selected[0])
        if path is None:
            return
        if path.is_dir():
            self.load_directory_async(path)
        else:
            self.load_directory_async(path.parent, select_path=path)

    def _on_tree_double_click(self, _event=None) -> None:
        selected = self.tree.selection()
        if not selected:
            return
        path = self.tree_path_by_id.get(selected[0])
        if path and path.is_file():
            open_with_system(path)

    def load_directory_async(self, directory: Path, select_path: Path | None = None) -> None:
        self.pending_content_token += 1
        current_token = self.pending_content_token
        self._start_activity()
        self.path_var.set(str(directory))

        def worker() -> None:
            try:
                entries = list_directory(directory)
            except Exception as exc:  # noqa: BLE001
                self.after(0, lambda: self._finish_directory_load_error(exc))
                return
            self.after(0, lambda: self._finish_directory_load(current_token, directory, entries, select_path))

        threading.Thread(target=worker, daemon=True).start()

    def _finish_directory_load_error(self, exc: Exception) -> None:
        self._stop_activity()
        messagebox.showerror("Error", str(exc), parent=self)

    def _finish_directory_load(self, token: int, directory: Path, entries, select_path: Path | None) -> None:
        self._stop_activity()
        if token != self.pending_content_token:
            return
        self.current_dir = directory
        self.current_entries = entries
        self._fill_content(entries, directory)
        if select_path is not None:
            for item_id, path in self.content_path_by_id.items():
                if path == select_path:
                    self.content_tree.selection_set(item_id)
                    self.content_tree.focus(item_id)
                    self.content_tree.see(item_id)
                    break
        self._update_info_for_content_selection()

    def _fill_content(self, entries, directory: Path, search_mode: bool = False) -> None:
        self.content_tree.delete(*self.content_tree.get_children())
        self.content_path_by_id.clear()
        for entry in entries:
            item_id = self.content_tree.insert(
                "",
                "end",
                text=entry.name,
                image=self.folder_icon if entry.is_dir else self.file_icon,
                values=(
                    "Carpeta" if entry.is_dir else entry.suffix.lstrip(".").upper() or "Archivo",
                    entry.size_label,
                    entry.modified_label,
                    str(entry.path.parent if search_mode else directory),
                ),
            )
            self.content_path_by_id[item_id] = entry.path
        self.path_var.set(str(directory))

    def _on_content_select(self, _event=None) -> None:
        self._update_info_for_content_selection()

    def _update_info_for_content_selection(self) -> None:
        paths = self.get_selected_content_paths()
        if not paths:
            if self.current_dir is not None:
                self.info_var.set(f"Carpeta actual: {self.current_dir}")
                self.info_icon_label.configure(image=self.folder_icon)
            else:
                self.info_var.set("Sin seleccion.")
                self.info_icon_label.configure(image=self.file_icon)
            return

        if len(paths) == 1:
            path = paths[0]
            if path.is_dir():
                self.info_var.set(f"[DIR] {path.name}")
                self.info_icon_label.configure(image=self.folder_icon)
            else:
                size = path.stat().st_size if path.exists() else 0
                self.info_var.set(f"[FILE] {path.name} | {format_size(size)}")
                self.info_icon_label.configure(image=self.file_icon)
            return

        total_file_size = 0
        for path in paths:
            if path.is_file() and path.exists():
                total_file_size += path.stat().st_size
        self.info_icon_label.configure(image=self.multi_icon)
        self.info_var.set(f"{len(paths)} elementos seleccionados | Tamano directo: {format_size(total_file_size)}")

    def _on_content_double_click(self, _event=None) -> None:
        selected = self.content_tree.selection()
        if not selected:
            return
        path = self.content_path_by_id.get(selected[0])
        if path is None:
            return
        if path.is_dir():
            self._select_tree_item(path)
            self.load_directory_async(path)
        else:
            open_with_system(path)

    def _on_content_right_click(self, event) -> None:
        row = self.content_tree.identify_row(event.y)
        if row:
            if row not in self.content_tree.selection():
                self.content_tree.selection_set(row)
            self.content_tree.focus(row)
        self._show_context_menu(event, source="content")

    def _on_tree_right_click(self, event) -> None:
        row = self.tree.identify_row(event.y)
        if row:
            self.tree.selection_set(row)
            self.tree.focus(row)
        self._show_context_menu(event, source="tree")

    def _show_context_menu(self, event, source: str) -> None:
        menu = tk.Menu(self, tearoff=False)
        selection = self.get_selection_from_source(source)
        target_dir = self.get_target_directory(source)

        if selection:
            menu.add_command(label="Abrir", command=lambda: self.open_selection(selection))
            if len(selection) == 1 and selection[0].is_file():
                menu.add_command(label="Abrir editor para editar fichero", command=lambda: self.open_with_dialog_action(selection[0]))
            menu.add_separator()
            menu.add_command(label="Copiar", command=lambda: self.set_clipboard(selection, "copy"))
            menu.add_command(label="Cortar", command=lambda: self.set_clipboard(selection, "cut"))
            if len(selection) == 1:
                menu.add_command(label="Renombrar", command=lambda: self.rename_action(selection[0]))
            menu.add_command(label="Eliminar", command=lambda: self.delete_action(selection))
            menu.add_separator()
            menu.add_command(label="Propiedades", command=lambda: self.properties_action(selection))

            archive_selection = [path for path in selection if is_archive(path)]
            if archive_selection and len(archive_selection) == len(selection):
                menu.add_separator()
                menu.add_command(label="Descomprimir aqui", command=lambda: self.extract_here_action(archive_selection))
                menu.add_command(label="Descomprimir...", command=lambda: self.extract_custom_action(archive_selection))

            chd_sources = gather_chd_sources_from_selection(selection)
            if chd_sources:
                menu.add_separator()
                menu.add_command(
                    label=self.build_chd_menu_label(selection, chd_sources),
                    command=lambda: self.convert_to_chd_action(selection, chd_sources),
                )

        if target_dir is not None:
            menu.add_separator()
            menu.add_command(label="Crear carpeta", command=lambda: self.create_folder_action(target_dir))
            menu.add_command(label="Crear fichero", command=lambda: self.create_file_action(target_dir))
            if self.clipboard_payload is not None:
                menu.add_command(label="Pegar", command=lambda: self.paste_action(target_dir))

        try:
            menu.tk_popup(event.x_root, event.y_root)
        finally:
            menu.grab_release()

    def build_chd_menu_label(self, selection: list[Path], chd_sources) -> str:
        only_folders = all(path.is_dir() for path in selection)
        only_files = all(path.is_file() for path in selection)
        ext_label = format_extension_labels(chd_sources)

        if only_folders:
            return "Convertir carpeta a .chd (analisis recursivo)" if len(selection) == 1 else "Convertir carpetas a .chd (analisis recursivo)"
        if only_files:
            return (
                f"Convertir fichero {ext_label} a .chd"
                if len(selection) == 1
                else f"Convertir ficheros {ext_label} a .chd"
            )
        return f"Convertir seleccion {ext_label} a .chd"

    def get_selection_from_source(self, source: str) -> list[Path]:
        if source == "tree":
            return [self.tree_path_by_id[item_id] for item_id in self.tree.selection() if item_id in self.tree_path_by_id]
        return self.get_selected_content_paths()

    def get_selected_content_paths(self) -> list[Path]:
        return [self.content_path_by_id[item_id] for item_id in self.content_tree.selection() if item_id in self.content_path_by_id]

    def get_target_directory(self, source: str) -> Path | None:
        selection = self.get_selection_from_source(source)
        if len(selection) == 1 and selection[0].is_dir():
            return selection[0]
        if len(selection) == 1 and selection[0].is_file():
            return selection[0].parent
        return self.current_dir

    def open_selection(self, selection: list[Path]) -> None:
        if not selection:
            return
        first = selection[0]
        if first.is_dir():
            self._select_tree_item(first)
            self.load_directory_async(first)
        else:
            open_with_system(first)

    def open_with_dialog_action(self, path: Path) -> None:
        open_with_dialog(path)

    def set_clipboard(self, paths: list[Path], operation: str) -> None:
        self.clipboard_payload = ClipboardPayload(paths=paths, operation=operation)
        self.info_var.set(f"{len(paths)} elemento(s) preparado(s) para {'mover' if operation == 'cut' else 'copiar'}.")

    def paste_action(self, target_dir: Path | None = None) -> None:
        if self.clipboard_payload is None:
            messagebox.showinfo("Pegar", "No hay nada en el portapapeles interno.", parent=self)
            return
        destination = target_dir or self.current_dir
        if destination is None:
            return

        overwrite = messagebox.askyesno(
            "Sobrescritura",
            "Si algun destino ya existe, deseas sobrescribirlo automaticamente?",
            parent=self,
        )

        payload = self.clipboard_payload

        def worker(progress, log) -> None:
            copy_or_move_paths(payload.paths, destination, payload.operation, overwrite, progress, log)

        def on_success() -> None:
            if payload.operation == "cut":
                self.clipboard_payload = None
            self.reload_current_view()

        self.run_operation_dialog("Copiar / mover", worker, on_success)

    def rename_action(self, path: Path) -> None:
        new_name = simpledialog.askstring("Renombrar", "Nuevo nombre:", initialvalue=path.name, parent=self)
        if not new_name or new_name == path.name:
            return
        try:
            rename_path(path, new_name)
        except Exception as exc:  # noqa: BLE001
            messagebox.showerror("Error", str(exc), parent=self)
            return
        self.reload_current_view()

    def delete_action(self, selection: list[Path]) -> None:
        if not selection:
            return
        if not messagebox.askyesno("Eliminar", f"Seguro que deseas eliminar {len(selection)} elemento(s)?", parent=self):
            return

        def worker(progress, log) -> None:
            remove_paths(selection, progress, log)

        self.run_operation_dialog("Eliminar", worker, self.reload_current_view)

    def properties_action(self, selection: list[Path]) -> None:
        if not selection:
            return
        summary = summarize_paths(selection)
        PropertiesDialog(self, "Propiedades", summary)

    def create_folder_action(self, target_dir: Path | None = None) -> None:
        destination = target_dir or self.current_dir
        if destination is None:
            return
        name = simpledialog.askstring("Crear carpeta", "Nombre de la carpeta:", parent=self)
        if not name:
            return
        try:
            create_folder(destination, name)
        except Exception as exc:  # noqa: BLE001
            messagebox.showerror("Error", str(exc), parent=self)
            return
        self.reload_current_view()

    def create_file_action(self, target_dir: Path | None = None) -> None:
        destination = target_dir or self.current_dir
        if destination is None:
            return
        name = simpledialog.askstring(
            "Crear fichero",
            "Nombre del fichero. Si no indicas extension, se asumira .txt:",
            parent=self,
        )
        if not name:
            return
        try:
            create_file(destination, name)
        except Exception as exc:  # noqa: BLE001
            messagebox.showerror("Error", str(exc), parent=self)
            return
        self.reload_current_view()

    def extract_here_action(self, archives: list[Path]) -> None:
        options = ExtractionOptions()
        self.start_extraction(archives, options)

    def extract_custom_action(self, archives: list[Path]) -> None:
        dialog = ExtractionOptionsDialog(self, archives, build_extraction_preview)
        self.wait_window(dialog)
        if dialog.result is None:
            return
        self.start_extraction(archives, dialog.result)

    def start_extraction(self, archives: list[Path], options: ExtractionOptions) -> None:
        def worker(progress, log) -> None:
            extract_archives(archives, options, progress, log)

        self.run_operation_dialog("Descomprimir", worker, self.reload_current_view)

    def convert_to_chd_action(self, selection: list[Path], chd_sources) -> None:
        folder_mode = any(path.is_dir() for path in selection)
        dialog = ChdConversionDialog(self, folder_mode=folder_mode)
        self.wait_window(dialog)
        if dialog.result is None:
            return

        selected_root = selection[0] if len(selection) == 1 and selection[0].is_dir() else None

        def worker(progress, log) -> None:
            run_chd_conversion(
                chd_sources,
                dialog.result,
                self.project_root,
                selected_root,
                progress,
                log,
            )

        self.run_operation_dialog("Convertir a CHD", worker, self.reload_current_view)

    def run_operation_dialog(self, title: str, worker, on_success=None) -> None:
        dialog = ProgressDialog(self, title)
        updates: queue.Queue[tuple[str, object, object | None]] = queue.Queue()
        success_state = {"value": False}

        def progress_callback(ratio: float, message: str) -> None:
            updates.put(("progress", ratio, message))

        def log_callback(message: str) -> None:
            updates.put(("log", message, None))

        def task() -> None:
            try:
                worker(progress_callback, log_callback)
            except Exception as exc:  # noqa: BLE001
                updates.put(("error", str(exc), None))
                return
            success_state["value"] = True
            updates.put(("done", "Operacion completada.", None))

        threading.Thread(target=task, daemon=True).start()

        def poll() -> None:
            while not updates.empty():
                kind, first, second = updates.get_nowait()
                if kind == "progress":
                    dialog.update_progress(float(first), str(second))
                elif kind == "log":
                    dialog.append_log(str(first))
                elif kind == "done":
                    dialog.complete(str(first))
                elif kind == "error":
                    dialog.fail(str(first))
            if not dialog.done:
                self.after(100, poll)
            elif success_state["value"] and on_success is not None:
                on_success()

        poll()

    def reload_current_view(self) -> None:
        if self.current_dir is not None:
            self.load_directory_async(self.current_dir)
        if self.root_path is not None:
            root_items = self.tree.get_children("")
            if root_items:
                self.loaded_tree_dirs.clear()
                self._populate_tree_children(root_items[0], self.root_path)

    def _on_content_keypress(self, event) -> None:
        if event.keysym in {"Up", "Down", "Left", "Right", "Return", "Escape"}:
            return
        if not event.char or not event.char.isprintable() or event.state & 0x4:
            return
        self.type_select_buffer += event.char.casefold()
        if self.type_select_after_id is not None:
            self.after_cancel(self.type_select_after_id)
        self.type_select_after_id = self.after(1000, self._clear_type_select_buffer)

        for item_id in self.content_tree.get_children(""):
            path = self.content_path_by_id[item_id]
            if path.name.casefold().startswith(self.type_select_buffer):
                self.content_tree.selection_set(item_id)
                self.content_tree.focus(item_id)
                self.content_tree.see(item_id)
                self._update_info_for_content_selection()
                break

    def _clear_type_select_buffer(self) -> None:
        self.type_select_buffer = ""
        self.type_select_after_id = None

    def _schedule_global_search(self, _event=None) -> None:
        if self.global_search_after_id is not None:
            self.after_cancel(self.global_search_after_id)
        self.global_search_after_id = self.after(250, self.run_global_search)

    def run_global_search(self) -> None:
        query = self.global_search_var.get().strip()
        self.global_search_after_id = None
        if not query:
            self.reload_current_view()
            return
        if self.root_path is None:
            return
        self.pending_search_token += 1
        current_token = self.pending_search_token
        self._start_activity()

        def worker() -> None:
            try:
                results = iter_search(self.root_path, query, recursive=True)
            except Exception as exc:  # noqa: BLE001
                self.after(0, lambda: self._finish_search_error(exc))
                return
            self.after(0, lambda: self._finish_global_search(current_token, query, results))

        threading.Thread(target=worker, daemon=True).start()

    def _finish_search_error(self, exc: Exception) -> None:
        self._stop_activity()
        messagebox.showerror("Error", str(exc), parent=self)

    def _finish_global_search(self, token: int, query: str, results) -> None:
        self._stop_activity()
        if token != self.pending_search_token or self.root_path is None:
            return
        self.current_entries = results
        self._fill_content(results, self.root_path, search_mode=True)
        self.path_var.set(f"Busqueda global '{query}' en {self.root_path}")
        self._update_info_for_content_selection()

    def open_local_search(self) -> None:
        if self.current_dir is None:
            return
        if self.local_search_window is not None and self.local_search_window.winfo_exists():
            self.local_search_window.lift()
            return

        window = tk.Toplevel(self)
        window.title("Buscar en carpeta actual")
        window.geometry("460x120")
        window.transient(self)
        self.local_search_window = window
        self.local_search_var.set("")
        self.local_recursive_var.set(False)

        frame = ttk.Frame(window, padding=12)
        frame.pack(fill="both", expand=True)
        ttk.Label(frame, text="Texto a buscar:").pack(anchor="w")
        entry = ttk.Entry(frame, textvariable=self.local_search_var)
        entry.pack(fill="x", pady=(4, 6))
        entry.focus_set()
        entry.bind("<KeyRelease>", self._schedule_local_search)
        ttk.Checkbutton(
            frame,
            text="Buscar recursivamente en subdirectorios",
            variable=self.local_recursive_var,
            command=self.run_local_search,
        ).pack(anchor="w")

        window.protocol("WM_DELETE_WINDOW", self.close_local_search)

    def _schedule_local_search(self, _event=None) -> None:
        if self.local_search_after_id is not None:
            self.after_cancel(self.local_search_after_id)
        self.local_search_after_id = self.after(250, self.run_local_search)

    def run_local_search(self) -> None:
        if self.current_dir is None:
            return
        query = self.local_search_var.get().strip()
        if self.local_search_after_id is not None:
            self.after_cancel(self.local_search_after_id)
            self.local_search_after_id = None
        if not query:
            self.load_directory_async(self.current_dir)
            return

        recursive = self.local_recursive_var.get()
        self.pending_search_token += 1
        current_token = self.pending_search_token
        base_dir = self.current_dir
        self._start_activity()

        def worker() -> None:
            try:
                results = iter_search(base_dir, query, recursive=recursive)
            except Exception as exc:  # noqa: BLE001
                self.after(0, lambda: self._finish_search_error(exc))
                return
            self.after(0, lambda: self._finish_local_search(current_token, base_dir, query, results))

        threading.Thread(target=worker, daemon=True).start()

    def _finish_local_search(self, token: int, base_dir: Path, query: str, results) -> None:
        self._stop_activity()
        if token != self.pending_search_token:
            return
        self.current_entries = results
        self._fill_content(results, base_dir, search_mode=True)
        scope = "recursiva" if self.local_recursive_var.get() else "en carpeta actual"
        self.path_var.set(f"Busqueda {scope} '{query}' en {base_dir}")
        self._update_info_for_content_selection()

    def close_local_search(self) -> None:
        self.clear_local_search_state(restore=True)

    def clear_local_search_state(self, restore: bool) -> None:
        if self.local_search_after_id is not None:
            self.after_cancel(self.local_search_after_id)
            self.local_search_after_id = None
        self.local_search_var.set("")
        self.local_recursive_var.set(False)
        if self.local_search_window is not None and self.local_search_window.winfo_exists():
            self.local_search_window.destroy()
        self.local_search_window = None
        if restore and self.current_dir is not None:
            self.load_directory_async(self.current_dir)

    def clear_global_search(self) -> None:
        self.global_search_var.set("")
        if self.current_dir is not None:
            self.load_directory_async(self.current_dir)

    def show_tools_info(self) -> None:
        detected = detect_chdman(self.project_root)
        detected_text = str(detected) if detected else "No detectado"
        metadata = load_chdman_metadata(self.project_root)
        linux_status = metadata.get("linux", {}).get("status", "Sin datos")
        messagebox.showinfo(
            "Herramientas",
            (
                "chdman detectado en:\n"
                f"{detected_text}\n\n"
                f"Estado Linux preparado:\n{linux_status}\n\n"
                "Para integrar binarios propios, usa third_party/chdman."
            ),
            parent=self,
        )

    def show_about(self) -> None:
        metadata = load_chdman_metadata(self.project_root)
        chdman_version = metadata.get("version", "Desconocida")
        release_tag = metadata.get("release_tag", "-")
        author = metadata.get("author", "MAMEdev and contributors")
        source_label = metadata.get("source", "Lanzamiento oficial de MAME")
        windows_status = metadata.get("windows", {}).get("status", "No verificado")
        linux_status = metadata.get("linux", {}).get("status", "No verificado")
        messagebox.showinfo(
            "Acerca de",
            (
                "scriptsManageRoms\n\n"
                "Explorador y gestor personal de ROMs orientado a Windows y Linux.\n\n"
                "Integracion CHD basada en chdman, herramienta oficial del proyecto MAME.\n"
                f"Version integrada: {chdman_version} ({release_tag})\n"
                f"Autoria/source: {author} | {source_label}\n"
                f"Estado Windows: {windows_status}\n"
                f"Estado Linux: {linux_status}\n"
                f"Documentacion oficial: {MAME_CHDMAN_DOCS_URL}\n"
                f"Descarga oficial de MAME: {MAME_RELEASE_URL}\n"
                f"Codigo fuente oficial: {MAME_GITHUB_URL}\n\n"
                "Agradecimiento al proyecto MAME por chdman y su documentacion oficial."
            ),
            parent=self,
        )

    def _select_tree_item(self, path: Path) -> None:
        for item_id, item_path in self.tree_path_by_id.items():
            if item_path == path:
                self.tree.selection_set(item_id)
                self.tree.focus(item_id)
                self.tree.see(item_id)
                return

        if self.root_path is None:
            return

        try:
            relative_parts = path.relative_to(self.root_path).parts
        except ValueError:
            return
        root_items = self.tree.get_children("")
        if not root_items:
            return
        current_item = root_items[0]
        current_path = self.root_path
        for part in relative_parts:
            self._populate_tree_children(current_item, current_path)
            next_item = None
            for child_id in self.tree.get_children(current_item):
                child_path = self.tree_path_by_id.get(child_id)
                if child_path and child_path.name == part:
                    next_item = child_id
                    current_path = child_path
                    break
            if next_item is None:
                return
            current_item = next_item

        self.tree.selection_set(current_item)
        self.tree.focus(current_item)
        self.tree.see(current_item)

    def _start_activity(self) -> None:
        self.activity_count += 1
        if self.activity_count == 1:
            self.activity_bar.start(12)

    def _stop_activity(self) -> None:
        self.activity_count = max(0, self.activity_count - 1)
        if self.activity_count == 0:
            self.activity_bar.stop()


def main() -> None:
    app = RomManagerApp()
    app.mainloop()
