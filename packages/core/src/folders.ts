export interface FolderCrumb {
  name: string;
  path: string;
}

/**
 * The folders from the filesystem root down to `path`, for the folder picker's
 * path bar: `"/media/Movies"` -> `/`, `media`, `Movies`, and
 * `"D:\\Films\\4K"` -> `D:`, `Films`, `4K`. The roots listing (`""`) has none.
 */
export function folderCrumbs(path: string): FolderCrumb[] {
  const drive = /^([A-Za-z]:)[\\/]?/.exec(path);
  const [root, sep] = drive ? [{ name: drive[1]!, path: `${drive[1]}\\` }, "\\"] : path.startsWith("/") ? [{ name: "/", path: "/" }, "/"] : [null, "/"];
  if (!root) return [];
  const crumbs = [root];
  let at = root.path;
  for (const name of path.slice(drive ? drive[0].length : 1).split(/[\\/]/).filter(Boolean)) {
    at = at.endsWith(sep) ? at + name : at + sep + name;
    crumbs.push({ name, path: at });
  }
  return crumbs;
}
