import { expect, it } from "vitest";
import i18n from "../../i18n";

const keys = [
  "list.tabList","list.tabRegister","list.errorLoad",
  "list.allAccounts","list.allSymbols","list.allDirections","list.allStatuses",
  "list.searchPlaceholder","list.fromDate","list.toDate","list.customFilters",
  "list.combine","list.combineAnd","list.combineOr",
  "list.opEquals","list.opNotEquals","list.opContains","list.opMin","list.opMax","list.opExists",
  "list.apply","list.reset",
  "list.colEntryTime","list.colExitTime","list.colSymbol","list.colDirection","list.colStatus",
  "list.colStrategy","list.colPnl","list.colR","list.details","list.loading","list.empty",
  "list.totalCount","list.prev","list.next","list.pageOf",
  "list.close","list.savedEdits","list.deleted","list.overrideAdded","list.overrideReverted",
  "list.assigned","list.attachmentAdded","list.riskStatus","list.effectiveDiff","list.noEffectiveDiff",
  "list.attachmentTooBig","list.attachmentTypeNotAllowed","list.attachmentUnlinked","list.unlink",
  "list.zoomOpen","list.integrity","list.integrityOk","list.integrityFailed","list.noPreview",
  "list.mime","list.sizeBytes","list.dimensions",
  "list.executions","list.manualExec","list.fillExec","list.assignedLabel","list.needs_assignment",
  "list.unassigned","list.assignTo","list.entryLegN","list.overrides","list.none","list.revert",
  "list.reverted","list.newValue","list.reason","list.addOverride","list.attachments",
  "list.customValues","list.entryLegs","list.exitLegs","list.planned","list.executed",
  "list.exitReason","list.save","list.delete","list.deleteReason","list.confirmDelete","list.cancel",
  "journal.buy","journal.sell","journal.statusOpen","journal.statusClosed","journal.statusCancelled",
  "journal.risk_pending","journal.risk_calculated","journal.risk_no_stop_loss","journal.risk_manual_risk",
  "journal.link_before_trade","journal.link_after_trade","journal.link_chart","journal.link_news","journal.link_other",
  "journal.plannedR","journal.initialStopLoss","journal.takeProfit","journal.note","journal.volume",
  "journal.stopLoss","journal.strategy","journal.timeframe","journal.session","journal.tags",
  "journal.emotions","journal.mistakes","journal.hint",
  "list.tabImport",
  "mtImport.title","mtImport.hint","mtImport.account","mtImport.file",
  "mtImport.noFile","mtImport.submit","mtImport.importing","mtImport.reportTitle",
  "mtImport.totalRows","mtImport.skipped","mtImport.imported","mtImport.duplicates","mtImport.errors",
  "mtImport.needsAssignment","mtImport.tradesCreated","mtImport.fileDuplicate","mtImport.warnings",
  "mtImport.done","mtImport.error","mtImport.tooBig","mtImport.typeNotAllowed","mtImport.noData",
];

it("every key used by list components resolves to Persian (not the key itself)", () => {
  const missing = keys.filter((k) => i18n.t(k) === k);
  expect(missing).toEqual([]);
});
