use crate::DeployPlan;

/// Pretty-print a deploy plan with cost estimates
pub fn format_plan(plan: &DeployPlan) -> String {
    let mut out = String::new();

    for change in &plan.creates {
        let cost_str = change
            .estimated_cost
            .map(|c| format!("~${:.0}/mo", c))
            .unwrap_or_default();
        out.push_str(&format!(
            "+ Create: {} ({})  {}\n",
            change.resource_type, change.detail, cost_str
        ));
    }

    for change in &plan.updates {
        out.push_str(&format!(
            "~ Update: {} ({})  no cost change\n",
            change.resource_type, change.detail
        ));
    }

    for change in &plan.deletes {
        let cost_str = change
            .estimated_cost
            .map(|c| format!("-${:.0}/mo", c))
            .unwrap_or_default();
        out.push_str(&format!(
            "- Delete: {} ({})  {}\n",
            change.resource_type, change.detail, cost_str
        ));
    }

    out.push_str(&format!(
        "─────────────────────────────────────────\n"
    ));
    out.push_str(&format!(
        "Estimated monthly delta: +${:.0}/mo\n",
        plan.estimated_monthly_cost
    ));

    out
}
