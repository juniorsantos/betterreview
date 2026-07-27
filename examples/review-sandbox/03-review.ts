export type ReviewState = "pending" | "approved" | "changes_requested";

export function reviewLabel(state: ReviewState): string {
  return state.replace("_", " ");
}
