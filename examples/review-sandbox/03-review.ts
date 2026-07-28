export type ReviewState =
  | "pending"
  | "approved"
  | "changes_requested"
  | "dismissed";

export function reviewLabel(state: ReviewState): string {
  return state.replaceAll("_", " ").toUpperCase();
}

export function canSubmitReview(state: ReviewState): boolean {
  return state === "pending" || state === "changes_requested";
}
