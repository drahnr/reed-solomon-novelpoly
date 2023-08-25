
use static_init::dynamic;

#[dynamic(0)]
pub static AFFT: AdditiveFFT = AdditiveFFT::initalize();


/// Additive FFT and inverse in the "novel polynomial basis"
#[allow(non_snake_case)]
pub struct AdditiveFFT {
    /// Multiplier form of twisted factors used in `AdditiveFFT`
    pub skews: [Multiplier; ONEMASK as usize], // skew_multiplier
    /// Factors used in formal derivative, actually all zero if field was constructed correctly.
    #[cfg(b_is_not_one)]
    pub B: [Multiplier; FIELD_SIZE >> 1],
}

/// Formal derivative of polynomial in the new?? basis
pub fn formal_derivative(cos: &mut [Additive], size: usize) {
	for i in 1..size {
		let length = ((i ^ i - 1) + 1) >> 1;
		for j in (i - length)..i {
			cos[j] ^= cos.get(j + length).copied().unwrap_or(Additive::ZERO);
		}
	}
	let mut i = size;
	while i < FIELD_SIZE && i < cos.len() {
		for j in 0..size {
			cos[j] ^= cos.get(j + i).copied().unwrap_or(Additive::ZERO);
		}
		i <<= 1;
	}
}

/// Formal derivative of polynomial in tweaked?? basis
#[allow(non_snake_case)]
pub fn tweaked_formal_derivative(codeword: &mut [Additive], n: usize) {
    #[cfg(b_is_not_one)]
    let B = unsafe { &AFFT.B };

    // We change nothing when multiplying by b from B.
	#[cfg(b_is_not_one)]
	for i in (0..n).into_iter().step_by(2) {
		let b = Multiplier(ONEMASK) - B[i >> 1];
		codeword[i] = codeword[i].mul(b);
		codeword[i + 1] = codeword[i + 1].mul(b);
	}

	formal_derivative(codeword, n);

	// Again changes nothing by multiplying by b although b differs here.
	#[cfg(b_is_not_one)]
	for i in (0..n).into_iter().step_by(2) {
		let b = B[i >> 1];
		codeword[i] = codeword[i].mul(b);
		codeword[i + 1] = codeword[i + 1].mul(b);
	}
}

/// This test ensure that b can safely be bypassed in tweaked_formal_derivative
#[cfg(b_is_not_one)]
#[test]
fn b_is_one() {
    let B = unsafe { &AFFT.B };
    fn test_b(b: Multiplier) {
        for x in 0..FIELD_SIZE {
            let x = Additive(x as Elt);
        	assert_eq!(x, x.mul(b));
        }
    }
    let mut old_b = None;
	for i in (0..FIELD_SIZE).into_iter().step_by(256) {
        let b = B[i >> 1];
        if old_b != Some(b) {
            test_b( Multiplier(ONEMASK) - b );
            test_b( b );
            old_b = Some(b);
        }
    }
}



// We want the low rate scheme given in
// https://www.citi.sinica.edu.tw/papers/whc/5524-F.pdf
// and https://github.com/catid/leopard/blob/master/docs/LowRateDecoder.pdf
// but this code resembles https://github.com/catid/leopard which
// implements the high rate decoder in
// https://github.com/catid/leopard/blob/master/docs/HighRateDecoder.pdf
// We're hunting for the differences and trying to undersrtand the algorithm.

/// Inverse additive FFT in the "novel polynomial basis"
pub fn inverse_afft(data: &mut [Additive], size: usize, index: usize) {
    unsafe { &AFFT }.inverse_afft(data,size,index)
}

pub fn inverse_afft_faster8(data: &mut [Additive8x], size: usize, index: usize) {
    unsafe { &AFFT }.inverse_afft_faster8(data,size,index)
}

/// Additive FFT in the "novel polynomial basis"
pub fn afft(data: &mut [Additive], size: usize, index: usize) {
    unsafe { &AFFT }.afft(data,size,index)
}

/// Additive FFT in the "novel polynomial basis"
pub fn afft_faster8(data: &mut [Additive8x], size: usize, index: usize) {
    unsafe { &AFFT }.afft_faster8(data,size,index)
}


impl AdditiveFFT {

    /// Inverse additive FFT in the "novel polynomial basis"
    pub fn inverse_afft(&self, data: &mut [Additive], size: usize, index: usize) {
    	// All line references to Algorithm 2 page 6288 of
    	// https://www.citi.sinica.edu.tw/papers/whc/5524-F.pdf

    	// Depth of the recursion on line 7 and 8 is given by depart_no
    	// aka 1 << ((k of Algorithm 2) - (i of Algorithm 2)) where
    	// k of Algorithm 1 is read as FIELD_BITS here.
    	// Recusion base layer implicitly imports d_r aka ala line 1.
    	// After this, we start at depth (i of Algorithm 2) = (k of Algorithm 2) - 1
    	// and progress through FIELD_BITS-1 steps, obtaining \Psi_\beta(0,0).
    	let mut depart_no = 1_usize;
    	while depart_no < size {
    		// Agrees with for loop (j of Algorithm 2) in (0..2^{k-i-1}) from line 3,
    		// except we've j in (depart_no..size).step_by(2*depart_no), meaning
    		// the doubled step compensated for the halve size exponent, and
    		// somehow this j captures the subscript on \omega_{j 2^{i+1}}.	 (TODO)
    		let mut j = depart_no;
    		while j < size {
    			// At this point loops over i in (j - depart_no)..j give a bredth
    			// first loop across the recursion branches from lines 7 and 8,
    			// so the i loop corresponds to r in Algorithm 2.  In fact,
    			// data[i] and data[i + depart_no] together cover everything,
    			// thanks to the outer j loop.

    			// Loop on line 3, so i corresponds to j in Algorithm 2
    			for i in (j - depart_no)..j {
    				// Line 4, justified by (34) page 6288, but
    				// adding depart_no acts like the r+2^i superscript.
    				data[i + depart_no] ^= data[i];
    			}

    			// Algorithm 2 indexs the skew factor in line 5 page 6288
    			// by i and \omega_{j 2^{i+1}}, but not by r explicitly.
    			// We further explore this confusion below. (TODO)
    			let skew = self.skews[j + index - 1];
    			// It's reasonale to skip the loop if skew is zero, but doing so with
    			// all bits set requires justification.	 (TODO)
    			if skew.0 != ONEMASK {
    				// Again loop on line 3, except skew should depend upon i aka j in Algorithm 2 (TODO)
    				for i in (j - depart_no)..j {
    					// Line 5, justified by (35) page 6288, but
    					// adding depart_no acts like the r+2^i superscript.
    					data[i] ^= data[i + depart_no].mul(skew);
    				}
    			}

    			// Increment by double depart_no in agreement with
    			// our updating 2*depart_no elements at this depth.
    			j += depart_no << 1;
    		}
    		depart_no <<= 1;
    	}
    }

    /// Inverse additive FFT in the "novel polynomial basis", but do 8 at once using available vector units
    pub fn inverse_afft_faster8(&self, data: &mut [Additive8x], size: usize, index: usize) {
    	let mut depart_no = size >> 1_usize;
		assert!(depart_no >= Additive8x::LANE);
		assert_eq!(depart_no % Additive8x::LANE, 0);

    	while depart_no < size {
    		let mut j = depart_no;
    		while j < size {
    			for i in ((j - depart_no)/Additive8x::LANE)..(j/Additive8x::LANE) {
    				data[i + depart_no/Additive8x::LANE] ^= data[i];
    			}
    			let skew = self.skews[j + index - 1];
    			if skew.0 != ONEMASK {
    				for i in ((j - depart_no)/Additive8x::LANE)..(j/Additive8x::LANE) {
    					data[i] ^= data[i + depart_no/Additive8x::LANE].mul(skew);
    				}
    			}
    			j += depart_no << 1;
    		}
    		depart_no <<= 1;
    	}
    }


    /// Additive FFT in the "novel polynomial basis"
    pub fn afft(&self, data: &mut [Additive], size: usize, index: usize) {
    	// All line references to Algorithm 1 page 6287 of
    	// https://www.citi.sinica.edu.tw/papers/whc/5524-F.pdf

    	// Depth of the recursion on line 3 and 4 is given by depart_no
    	// aka 1 << ((k of Algorithm 1) - (i of Algorithm 1)) where
    	// k of Algorithm 1 is read as FIELD_BITS here.
    	// Recusion base layer implicitly imports d_r aka ala line 1.
    	// After this, we start at depth (i of Algorithm 1) = (k of Algorithm 1) - 1
    	// and progress through FIELD_BITS-1 steps, obtaining \Psi_\beta(0,0).
    	let mut depart_no = size >> 1_usize;
    	while depart_no > 0 {
    		// Agrees with for loop (j of Algorithm 1) in (0..2^{k-i-1}) from line 5,
    		// except we've j in (depart_no..size).step_by(2*depart_no), meaning
    		// the doubled step compensated for the halve size exponent, and
    		// somehow this j captures the subscript on \omega_{j 2^{i+1}}.	 (TODO)
    		let mut j = depart_no;
    		while j < size {
				// At this point loops over i in (j - depart_no)..j give a bredth
    			// first loop across the recursion branches from lines 3 and 4,
    			// so the i loop corresponds to r in Algorithm 1.  In fact,
    			// data[i] and data[i + depart_no] together cover everything,
    			// thanks to the outer j loop.

    			// Algorithm 1 indexs the skew factor in line 6 aka (28) page 6287
    			// by i and \omega_{j 2^{i+1}}, but not by r explicitly.
    			// We doubt the lack of explicit dependence upon r justifies
    			// extracting the skew factor outside the loop here.
    			// As indexing by \omega_{j 2^{i+1}} appears absolute elsewhere,
    			// we think r actually appears but the skew factor repeats itself
    			// like in (19) in the proof of Lemma 4.  (TODO)
    			// We should understand the rest of this basis story, like (8) too.	 (TODO)
    			let skew = self.skews[j + index - 1];

				// It's reasonale to skip the loop if skew is zero, but doing so with
    			// all bits set requires justification.	 (TODO)
    			if skew.0 != ONEMASK {
    				// Loop on line 5, except skew should depend upon i aka j in Algorithm 1 (TODO)
    				for i in (j - depart_no)..j {
    					// Line 6, explained by (28) page 6287, but
    					// adding depart_no acts like the r+2^i superscript.
						data[i] ^= data[i + depart_no].mul(skew);
    				}
    			}

    			// Again loop on line 5, so i corresponds to j in Algorithm 1
    			for i in (j - depart_no)..j {
    				// Line 7, explained by (31) page 6287, but
    				// adding depart_no acts like the r+2^i superscript.
					data[i + depart_no] ^= data[i];
    			}

    			// Increment by double depart_no in agreement with
    			// our updating 2*depart_no elements at this depth.
    			j += depart_no << 1;
    		}
    		depart_no >>= 1;
    	}
    }
    /// Additive FFT in the "novel polynomial basis", but do 8 at once using available vector units
	///
	/// `size` is the count of the individual additive field elements, so 8x larger than `data.len()`.
    pub fn afft_faster8(&self, data: &mut [Additive8x], size: usize, index: usize) {
    	let mut depart_no = size >> 1_usize;
    	while depart_no >= Additive8x::LANE {
    		let mut j = depart_no;
    		while j < size {
				let skew = self.skews[j + index - 1];
				// correct as long as `depart_no` is equal or larger to `Additive8x::LANE`
				let j_8x = j/Additive8x::LANE;
				let depart_no_8x = depart_no / Additive8x::LANE;

    			if skew.0 != ONEMASK {
    				for i in (j_8x - depart_no_8x)..j_8x {
    					data[i] ^= data[i + depart_no_8x].mul(skew);
    				}
    			}

    			for i in (j_8x - depart_no_8x)..j_8x {
    				data[i + depart_no_8x] ^= data[i];
    			}

    			j += depart_no << 1;
    		}
			depart_no >>= 1;
    	}
		// the above is correct, the mistake is here:

		// do the regular for the smaller ones, but inline it so we don't need to allocate anything
		// and load the values from the avx registers as needed
    	while depart_no > 0 {
    		let mut j = depart_no;
    		while j < size {
    			println!("Round {j} @ depart_no {depart_no}");

				let skew = self.skews[j + index - 1];
    			let offset = (j - depart_no)/Additive8x::LANE;

				// calculate the remainder offset, which is within the selected `[Additive; 8]` slice
				let intra = (j - depart_no) - offset * Additive8x::LANE;

				let mut local_data = Additive8x::unpack(&data[offset]);

				if skew.0 != ONEMASK {
    				for i in intra..(intra + depart_no) {
    					local_data[i] ^= local_data[i + depart_no].mul(skew);
    				}
    			}

				for i in intra..(intra + depart_no) {
    				local_data[i + depart_no] ^= local_data[i];
    			}

				data[offset].override_partial_continuous(0, &local_data[..(depart_no * 2)]);

				j += depart_no << 1;
    		}
    		depart_no >>= 1;
    	}
    }

    //initialize SKEW_FACTOR and B
    fn initalize() -> AdditiveFFT {
        // We cannot yet identify if base has an additive or multiplicative
        // representation, or mybe something else entirely.  (TODO)
    	let mut base: [Elt; FIELD_BITS - 1] = Default::default();

        let mut skews_additive = [Additive(0); ONEMASK as usize];

    	for i in 1..FIELD_BITS {
    		base[i - 1] = 1 << i;
    	}

    	// We construct SKEW_FACTOR in additive form to be \bar{s}_j(omega)
    	// from page 6285 for all omega in the field.
    	for m in 0..(FIELD_BITS - 1) {
    		let step = 1 << (m + 1);
    		skews_additive[(1 << m) - 1] = Additive(0);
    		for i in m..(FIELD_BITS - 1) {
    			let s = 1 << (i + 1);
				// TODO if s>=8 we cound employ SIMD
				// TODO and step % 8 == 0
    			let mut j = (1 << m) - 1;
    			while j < s {
    				// Justified by (5) page 6285, except..
    				// we expect SKEW_FACTOR[j ^ field_base[i]] or similar
    				skews_additive[j + s] = skews_additive[j] ^ Additive(base[i]);
    				j += step;
    			}
    		}

    		// Compute base[m] = ONEMASK - base[m] * EXP[LOG[base[m] ^ 1]]
    		// = ONEMASK - base[m] * (base[m] ^ 1)
    		// TODO: But why?
    		//
    		// let idx = mul_table(base[m], LOG_TABLE[(base[m] ^ 1_u16) as usize]);
    		let idx = Additive(base[m]).mul( Additive(base[m] ^ 1).to_multiplier() );
            // WTF?!?
    		// base[m] = ONEMASK - LOG_TABLE[idx as usize];
            base[m] = ONEMASK - idx.to_multiplier().0;

    		// Compute base[i] = base[i] * EXP[b % ONEMASK]
    		// where b = base[m] + LOG[base[i] ^ 1_u16].
    		// As ONEMASK is the order of the multiplicative grou,
    		// base[i] = base[i] * EXP[base[m]] * (base[i] ^ 1)
    		// TODO: But why?
    		for i in (m + 1)..(FIELD_BITS - 1) {
    			// WTF?!?
    			// let b = LOG_TABLE[(base[i] as u16 ^ 1_u16) as usize] as u32 + base[m] as u32;
    			let b = Additive(base[i] ^ 1).to_multiplier().to_wide() + (base[m] as Wide);
    			let b = b % (ONEMASK as Wide);
    			// base[i] = mul_table(base[i], b as u16);
    			base[i] = Additive(base[i]).mul(Multiplier(b as Elt)).0;
    		}
    	}

    	// Convert skew factors from Additive to Multiplier form
        let mut skews_multiplier = [Multiplier(0); ONEMASK as usize];
    	for i in 0..(ONEMASK as usize) {
    		// SKEW_FACTOR[i] = LOG_TABLE[SKEW_FACTOR[i] as usize];
    		skews_multiplier[i] = skews_additive[i].to_multiplier();
    	}

        AdditiveFFT {
            // skews_additive,
            skews: skews_multiplier,
			#[cfg(b_is_not_one)]
			B: {
                let mut B = [Multiplier(0); FIELD_SIZE >> 1];

            	// TODO: How does this alter base?
            	base[0] = ONEMASK - base[0];
            	for i in 1..(FIELD_BITS - 1) {
            		base[i] = ( (
                        (ONEMASK as Wide) - (base[i] as Wide) + (base[i - 1] as Wide)
                    ) % (ONEMASK as Wide) ) as Elt;
            	}

            	// TODO: What is B anyways?
            	B[0] = Multiplier(0);
            	for i in 0..(FIELD_BITS - 1) {
            		let depart = 1 << i;
            		for j in 0..depart {
            			B[j + depart] = Multiplier( ((
                            B[j].to_wide() + (base[i] as Wide)
                        ) % (ONEMASK as Wide)) as Elt);
            		}
            	}

                B
            }
        }
    }

}

#[cfg(test)]
mod afft_tests {
	use super::*;
	use rand::{Rng,SeedableRng};
	use rand::rngs::SmallRng;

	fn gen_plain<R: Rng + SeedableRng<Seed = [u8; 32]>>(size: usize) -> Vec<Additive> {
		let mut rng = <R as SeedableRng>::from_seed(reed_solomon_tester::SMALL_RNG_SEED);
		let dist = rand::distributions::Uniform::new_inclusive(u16::MIN, u16::MAX);
		let data = Vec::from_iter(rng.sample_iter::<u16, _>(dist.clone()).take(size).map(Additive));
		data
	}

	fn gen_faster8<R: Rng + SeedableRng<Seed = [u8; 32]>>(size: usize) -> Vec<Additive8x> {
		let data = gen_plain::<R>(size);
		let mut dest = Vec::with_capacity((size+Additive8x::LANE-1) / Additive8x::LANE);
		unsafe {
			dest.set_len(dest.capacity());
		}
		convert_to_faster8(&data, &mut dest);
		dest
	}

	#[test]
	fn conversions_work() {
		let data = gen_plain::<SmallRng>(256);
		let mut faster8 = vec![Additive8x::zero(); 32];
		convert_to_faster8(&data, &mut faster8);

		let mut flattened = vec![Additive::zero(); data.len()];
		convert_from_faster8(&faster8, &mut flattened);

		assert_eq!(flattened, data);

		assert_plain_eq_faster8(data, faster8.as_slice());
	}

	fn assert_plain_eq_faster8(plain: impl AsRef<[Additive]>, faster8: impl AsRef<[Additive8x]>) {
		let plain = plain.as_ref();
		let faster8 = faster8.as_ref();

		let mut faster8_flattened = vec![Additive::zero(); faster8.len() * Additive8x::LANE];
		convert_from_faster8(faster8, &mut faster8_flattened);
		itertools::assert_equal(plain, faster8_flattened.iter());
	}

	#[test]
	fn afft_output__plain_eq_faster8() {
		let index = 0;
		let size = 8;
		let mut data_plain = gen_plain::<SmallRng>(size);
		let mut data_faster8 = gen_faster8::<SmallRng>(size);
		println!(">>>>");
		unsafe { &AFFT }.afft(&mut data_plain, size, index);
		println!(r#"

		>>>>

		"#);
		unsafe { &AFFT }.afft_faster8(&mut data_faster8, size, index);
		println!(">>>>");
		assert_plain_eq_faster8(data_plain, data_faster8);
	}

	#[cfg(b_is_not_one)]
	#[test]
	fn b_is_one() {
		// This test ensure that b can be safely bypassed in tweaked_formal_derivative
		let B = unsafe { &AFFT.B };
		fn test_b(b: Multiplier) {
			for x in 0..FIELD_SIZE {
				let x = Additive(x as Elt);
				assert_eq!(x, x.mul(b));
			}
		}

		let mut old_b = None;

		for i in (0..FIELD_SIZE).into_iter().step_by(256) {
			let b = B[i >> 1];
			if old_b != Some(b) {
				test_b( Multiplier(ONEMASK) - b );
				test_b( b );
				old_b = Some(b);
			}
		}
	}

}
