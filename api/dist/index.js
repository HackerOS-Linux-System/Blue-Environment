var KNOWN_PERMISSIONS = ["notifications", "clipboard", "storage", "network", "files", "system"];
function defineBlueApp(mount) {
  return mount;
}
function hasPermission(api, permission) {
  return api.app.permissions.includes(permission);
}
export {
  KNOWN_PERMISSIONS,
  defineBlueApp,
  hasPermission
};
