// SPDX-License-Identifier: MIT
export function downloadBytes(bytes, name) {
    if (typeof document === "undefined") return bytes;
    const a = document.createElement("a");
    const url = URL.createObjectURL(new Blob([bytes]));
    a.href = url;
    a.download = name || (this.getAttribute("src") || "").split(/[?#]/)[0].split("/").pop() || "workbook.xlsx";
    a.click();
    URL.revokeObjectURL(url);
    return bytes;
  }
