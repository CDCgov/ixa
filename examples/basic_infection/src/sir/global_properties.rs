eosim::define_global_property!(R0, f64);

eosim::define_global_property!(InfectiousPeriod, f64);

eosim::define_global_property!(LatentPeriod, f64);

eosim::define_global_property!(SymptomaticPeriod, f64);

eosim::define_global_property!(IncubationPeriod, f64);

eosim::define_global_property!(HospitalizationDuration, f64);

eosim::define_global_property!(ProbabilityHospitalized, f64);

eosim::define_global_property!(HospitalizationDelay, f64);

eosim::define_global_property!(ProbabilitySymptoms, f64);

eosim::define_global_property!(Population, usize);

eosim::define_global_property!(InitialInfections, usize);

eosim::define_global_property!(DeathRate, f64);

eosim::define_global_property!(MaxDays, usize);
