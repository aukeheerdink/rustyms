#![allow(clippy::missing_panics_doc)]
use std::io::BufReader;

use super::IdentifiedPeptideSource;

use super::{csv::parse_csv_raw, sage, IdentifiedPeptide, SageData};

#[test]
fn sage() {
    let reader = BufReader::new(DATA.as_bytes());
    let lines = parse_csv_raw(reader, b'\t', None).unwrap();
    for line in lines.map(Result::unwrap) {
        println!("{line}");
        let _read: IdentifiedPeptide = SageData::parse_specific(&line, &sage::VERSION_0_14, None)
            .unwrap()
            .into();
    }
}

const DATA: &str = r"psm_id	peptide	proteins	num_proteins	filename	scannr	rank	label	expmass	calcmass	charge	peptide_len	missed_cleavages	semi_enzymatic	isotope_error	precursor_ppm	fragment_ppm	hyperscore	delta_next	delta_best	rt	aligned_rt	predicted_rt	delta_rt_model	ion_mobility	predicted_mobility	delta_mobility	matched_peaks	longest_b	longest_y	longest_y_pct	matched_intensity_pct	scored_candidates	poisson	sage_discriminant_score	posterior_error	spectrum_q	peptide_q	protein_q	ms2_intensity
68	Q[-17.027]VQLQQSAAE	anti-FLAG-M2_HC	1	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_standard.mzML	controllerType=0 controllerNumber=1 scan=13947	1	1	1083.5209	1083.5192	2	10	0	0	0.0	1.5772523	4.326962	48.89544628732507	3.804374154620227	0.0	29.031733	0.4219151	0.42962697	0.0077118576	0.0	0.0	0.0	15	6	6	0.6	52.72756	48	-5.310279475246126	-0.2721751	-30.489534	0.003984064	0.013706031	1.0	2537541.5
258	AGNTFTCSVLHE	139H2_HC;anti-FLAG-M2_HC	2	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_exBusHCD.mzML	controllerType=0 controllerNumber=1 scan=18157	1	1	1277.5806	1277.571	2	12	0	0	0.0	7.4527745	5.461328	29.359412403792973	29.359412403792973	0.0	30.288746	0.61734784	0.62651765	0.009169817	0.0	0.0	0.0	9	2	5	0.41666666	44.847633	1	-1.3794106637415624	-0.27700272	-30.085367	0.003984064	0.013706031	1.0	37966.004
297	ATHKTSTSPIVKSFNR[+14.016]NE[+57.0214]	139H2_LC;anti-FLAG-M2_LC	2	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_exBUstandard.mzML	controllerType=0 controllerNumber=1 scan=40290	1	1	2087.0747	2087.0762	3	18	0	0	0.0	0.7018643	4.2632957	45.554752225675415	0.0	0.0	51.26408	0.89359313	0.89359313	0.0	0.0	0.0	0.0	16	4	5	0.2777778	48.30265	147	-6.97961068204291	-0.29033038	-28.985697	0.003984064	0.013706031	1.0	144385.33
250	Q[-17.027]VQLQQSAAE	anti-FLAG-M2_HC	1	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_exBUstandard.mzML	controllerType=0 controllerNumber=1 scan=11449	1	1	1083.5221	1083.5192	2	10	0	0	0.0	2.7038596	4.253701	40.055635259043456	3.594351101362662	0.0	22.185923	0.4098663	0.42962697	0.019760668	0.0	0.0	0.0	13	6	5	0.5	39.98955	46	-5.091311704249276	-0.29421294	-28.668184	0.003984064	0.013706031	1.0	216813.19
34	Q[-17.027]VQLQQSGAE	139H2_HC	1	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_standard.mzML	controllerType=0 controllerNumber=1 scan=13201	1	1	1069.5068	1069.5035	2	10	0	0	0.0	3.0817041	4.7112064	42.145033139534505	7.121257426386592	0.0	27.521046	0.40820804	0.4030738	0.0051342547	0.0	0.0	0.0	14	6	5	0.5	30.92082	29	-6.025834389552996	-0.3012659	-28.096254	0.003984064	0.013706031	1.0	226430.94
201	DIVM[+15.9949]SQSPSSLAVSVGE	139H2_LC	1	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_exBusHCD.mzML	controllerType=0 controllerNumber=1 scan=16773	1	1	1720.8225	1720.8188	2	17	0	0	0.0	2.128118	5.3211784	44.885623333810656	2.3815242959373677	0.0	28.677181	0.5936372	0.60655576	0.012918532	0.0	0.0	0.0	16	6	5	0.29411766	40.6799	173	-9.665309113459845	-0.332584	-25.624058	0.003984064	0.013706031	1.0	94674.89
226	Q[-17.027]VQLQQSAAE	anti-FLAG-M2_HC	1	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_exBUstandard.mzML	controllerType=0 controllerNumber=1 scan=11301	1	1	1083.5217	1083.5192	2	10	0	0	0.0	2.3658774	5.314282	40.061219947915205	3.550720861949152	0.0	21.94597	0.4058746	0.42962697	0.023752362	0.0	0.0	0.0	13	6	5	0.5	48.80738	48	-5.5716324626825235	-0.3330954	-25.582886	0.003984064	0.013706031	1.0	215946.81
222	DIVM[+15.9949]SQSPSSLAVSVGE	139H2_LC	1	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_exBusHCD.mzML	controllerType=0 controllerNumber=1 scan=16952	1	1	1720.8245	1720.8188	2	17	0	0	0.0	3.2631123	5.1479435	38.36399763825078	2.353645184534386	0.0	28.915075	0.5971373	0.60655576	0.009418488	0.0	0.0	0.0	13	6	3	0.1764706	34.576347	66	-5.175294143112638	-0.35454315	-23.955624	0.003984064	0.013706031	1.0	63050.15
224	DIVM[+15.9949]SQSPSSLAVSVGE	139H2_LC	1	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_standard.mzML	controllerType=0 controllerNumber=1 scan=22930	1	1	1720.824	1720.8188	2	17	0	0	0.0	2.979364	5.0436964	55.80943198592773	2.5457567920643243	0.0	47.34651	0.5880926	0.60655576	0.018463135	0.0	0.0	0.0	20	6	5	0.29411766	39.965954	333	-11.506200275837854	-0.3676138	-22.99136	0.003984064	0.013706031	1.0	221128.81
13	[+27.995]-VPYTFGGGTKLE	131-2a_LC	1	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_exBUstandard.mzML	controllerType=0 controllerNumber=1 scan=24000	1	1	1295.6453	1295.6399	2	12	0	0	0.0	4.145506	6.2266393	31.96070176878828	0.0	0.0	38.387306	0.6793828	0.725527	0.046144187	0.0	0.0	0.0	10	3	5	0.41666666	27.924307	7	-2.5168127110893543	-0.3680259	-22.96079	0.003984064	0.013706031	1.0	50903.387
90	LNSLTSEDSAVYYC[+57.0214]AR[+0.984016]E	anti-FLAG-M2_HC	1	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_standard.mzML	controllerType=0 controllerNumber=1 scan=33562	1	1	1977.869	1977.8623	2	17	1	0	0.0	3.394501	4.2286243	39.65848601439715	0.0	0.0	68.27057	0.77794516	0.77793664	0.000008523464	0.0	0.0	0.0	13	4	9	0.5294118	63.78223	880	-6.321589862290821	-0.37199542	-22.671942	0.003984064	0.013706031	1.0	150724.14
234	Q[-17.027]VQLQQSAAE	anti-FLAG-M2_HC	1	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_exBusHCD.mzML	controllerType=0 controllerNumber=1 scan=11298	1	1	1083.5231	1083.5192	2	10	0	0	0.0	3.6051443	4.347138	40.03368128463812	2.1763350463467646	0.0	22.059513	0.49627298	0.42962697	0.06664601	0.0	0.0	0.0	13	6	4	0.4	37.836575	40	-4.514666428409349	-0.392876	-21.18462	0.003984064	0.013706031	1.0	218509.98
260	DIVMSQSPSSLAVSVGE	139H2_LC	1	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_exBUstandard.mzML	controllerType=0 controllerNumber=1 scan=19637	1	1	1704.8264	1704.824	2	17	0	0	0.0	1.4320567	3.6145332	49.230159762961	15.410105905845562	0.0	33.09084	0.591274	0.5662531	0.025020897	0.0	0.0	0.0	15	6	5	0.29411766	42.009964	94	-9.333262767059562	-0.39697143	-20.898802	0.003984064	0.013706031	1.0	2413815.5
46	VHTAQTQPR[+0.984016]EE-[-0.984016]	139H2_HC;anti-FLAG-M2_HC	2	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_exBUstandard.mzML	controllerType=0 controllerNumber=1 scan=6001	1	1	1294.6272	1294.6266	3	11	1	0	0.0	0.47144976	4.4562635	55.44425044464235	2.2673195462646163	0.0	12.45119	0.24792513	0.601642	0.35371688	0.0	0.0	0.0	18	5	5	0.45454547	51.329735	4	-1.7995540993010308	-0.39836758	-20.801445	0.003984064	0.013706031	1.0	2194955.5
40	DIVMSQSPSSLAVSVGE	139H2_LC	1	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_standard.mzML	controllerType=0 controllerNumber=1 scan=26843	1	1	1704.8295	1704.824	2	17	0	0	0.0	3.2221246	3.4901772	49.18510235659944	14.423783669103138	0.0	55.149353	0.658891	0.5662531	0.0926379	0.0	0.0	0.0	16	6	5	0.29411766	39.335335	107	-9.10993737230424	-0.402913	-20.48709	0.003984064	0.013706031	1.0	721527.0
304	DIVM[+15.9949]SQSPSSLAVSVGE	139H2_LC	1	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_exBUstandard.mzML	controllerType=0 controllerNumber=1 scan=16662	1	1	1720.8226	1720.8188	2	17	0	0	0.0	2.199055	6.3346987	52.99793412574644	2.567776726490308	0.0	29.34575	0.5289729	0.60655576	0.077582836	0.0	0.0	0.0	19	6	5	0.29411766	42.25518	277	-10.613723235983853	-0.40672565	-20.22537	0.003984064	0.013706031	1.0	169054.62
212	DIVM[+15.9949]SQSPSSLAVSVGE	139H2_LC	1	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_standard.mzML	controllerType=0 controllerNumber=1 scan=22811	1	1	1720.8243	1720.8188	2	17	0	0	0.0	3.1921754	6.0723367	55.856323439043265	2.6360479423688616	0.0	47.106743	0.5859171	0.60655576	0.020638645	0.0	0.0	0.0	20	6	5	0.29411766	40.340023	347	-13.02535171263656	-0.41279852	-19.81235	0.003984064	0.013706031	1.0	199134.47
29	DIVMSQSPSSLAVSVGE	139H2_LC	1	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_standard.mzML	controllerType=0 controllerNumber=1 scan=26712	1	1	1704.8274	1704.824	2	17	0	0	0.0	2.0048788	3.9185774	52.31447120292709	16.684994905824247	0.0	54.908207	0.656703	0.5662531	0.09044987	0.0	0.0	0.0	17	6	5	0.29411766	38.434063	91	-9.754505374647184	-0.42236862	-19.170193	0.003984064	0.013706031	1.0	1012092.44
231	DIVM[+15.9949]SQSPSSLAVSVGE	139H2_LC	1	20240113_EX3_UM5_Peng0013_SA_EXT00_GluC_2h_standard.mzML	controllerType=0 controllerNumber=1 scan=23044	1	1	1720.8228	1720.8188	2	17	0	0	0.0	2.2699924	5.326675	43.41870813056693	2.4671118628491797	0.0	47.584812	0.59025484	0.60655576	0.016300917	0.0	0.0	0.0	15	6	4	0.23529412	43.65104	214	-9.511001335763002	-0.42716968	-18.852083	0.003984064	0.013706031	1.0	99365.164";
