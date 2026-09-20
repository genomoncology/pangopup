# Cache rule coverage keeps its current runtime

Ian closed Ticket 0146 without implementation on 2026-09-20. The two mutation harnesses take about twelve seconds together on the measured Mac. They pass, they run only in repository testing, and they do not affect PangoPup lookup, inference, startup, or release artifact performance. Twelve seconds does not justify added test infrastructure or maintenance.

No code, fixture, budget, or gate changed. Draft 0109 remains archived with the ticket as the evidence that prompted the measurement. A future problem should open a new ticket only if these checks become unreliable or materially impede ordinary development.
