import {budgetStatus} from '../private-inference/client.ts';
export function createBudgetMonitor(state, render) {
  let pending=false;
  return async function refreshBudget() {
    if(pending)return;
    pending=true;
    try {
      const data=await budgetStatus();
      const provider=data.provider;
      const credit=typeof provider?.balanceMicros==='number'
        ? `$${(provider.balanceMicros/1e6).toFixed(2)} (${provider.status})` : 'unavailable';
      const remaining=(data.local.remainingMicros/1e6).toFixed(2);
      const cap=(data.local.capMicros/1e6).toFixed(2);
      const actual=(data.local.actualSpentMicros/1e6).toFixed(2);
      const holds=(data.local.reservedMicros/1e6).toFixed(2);
      state.budgetLine=`Runpod ${credit} | Cap remaining $${remaining} / $${cap} | Actual $${actual} · Holds $${holds}`;
      const warning=provider.lowBalance||data.lowAllowance;
      if(warning&&state.budgetWarning!==state.budgetLine) {
        state.traceLogs.push({type:'BUDGET',text:`Low budget: ${state.budgetLine}. Actual Runpod spend is reconciled against the cap; holds are unresolved in-flight reservations. Last provider observation: ${provider.observedAt??'unavailable'}.`,time:new Date().toLocaleTimeString()});
        state.budgetWarning=state.budgetLine;
      }
    } catch {state.budgetLine='Runpod credit unavailable | budget status could not be read';}
    finally {pending=false;render();}
  };
}
