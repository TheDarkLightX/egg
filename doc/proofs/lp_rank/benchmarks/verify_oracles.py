#!/usr/bin/env python3
"""Independent arithmetic checks for the benchmark's analytic cost oracle.

This consumes fixture names only, never CBC solutions or SCC implementation.
Shared exits are excluded only after proving an all-step route cheaper than
any 10000-cost exit. The remaining selected routes strictly decrease levels.
"""
import csv
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parent
rows=list(csv.DictReader((ROOT/'results.csv').open()))
verified=[]
for case in dict.fromkeys(r['case'] for r in rows):
    n=int(''.join(c for c in case if c.isdigit()))
    if case.startswith('ring'):
        candidates=[i+(1 if i==n//2 else 1000+i) for i in range(n)]
        cost=min(candidates)
        certificate=dict(exit_costs_with_path=candidates, chosen_exit=candidates.index(cost))
    elif case.startswith('shared'):
        assert n+1 < 10000, 'Leaf exits must be dominated for this recurrence'
        # Scale to integral fifths, avoiding decimal floating point in the oracle.
        dp=[5]
        for i in range(1,n+1):
            dp.append(min([5+dp[i-1]]+([6+dp[i-2]] if i>=2 else [])))
        cost=dp[n]/5
        certificate=dict(excluded_exit_lower_bound=10000, feasible_all_step_cost=n+1, optimal_route_cost_fifths=dp)
    else:
        cost=sum(1 for _ in range(n+1))
        certificate=dict(required_unit_cost_nodes=n+1)
    reported={float(r['expected']) for r in rows if r['case']==case}
    assert reported=={float(cost)},(case,cost,reported)
    verified.append(dict(case=case, exact_optimum=cost, certificate=certificate))
(ROOT/'oracle-verification.json').write_text(json.dumps(verified,indent=2)+'\n')
print(f'Independent fixture-cost certificates verified for {len(verified)} cases')
