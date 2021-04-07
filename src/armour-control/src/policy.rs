/*
 * Copyright (c) 2021 Arm Limited.
 *
 * SPDX-License-Identifier: MIT
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

use armour_lang::{
    expressions,
    interpret::{CPEnv, TExprInterpreter},
    literals::{self, CPFlatLiteral, CPLiteral},
    policies::{self, ONBOARDING_SERVICES},
};
use futures::future::{BoxFuture, FutureExt};
use std::sync::Arc;
use super::interpret::*;
use super::State;

pub struct OnboardingPolicy{
    policy: policies::OnboardingPolicy,
    env: CPEnv,
}

impl OnboardingPolicy{
    pub fn new(pol : policies::OnboardingPolicy) -> Self {
        let env = CPEnv::new(&pol.program);
        OnboardingPolicy {
            policy: pol,
            env,
        }
    }

    pub fn policy(&self) -> policies::OnboardingPolicy {
        self.policy.clone()
    }

    pub fn env(&self) -> &CPEnv {
        &self.env
    }

    pub fn evaluate(//<T: std::convert::TryFrom<literals::CPLiteral> + Send + 'static>(
        &self,
        state: State,
        onboarding_data: expressions::CPExpr,//onboardingData
    ) -> BoxFuture<'static, Result<Box<literals::OnboardingResult>, expressions::Error>> {
        log::debug!("evaluating onboarding service policy");
        let now = std::time::Instant::now();
        let env =self.env.clone(); 

        async move {
            let result = CPExprWrapper::evaluate(
                    expressions::Expr::call(ONBOARDING_SERVICES, vec!(onboarding_data)),
                    Arc::new(state), 
                    env.clone())
                .await?;

            log::debug!("result ({:?}): {}", now.elapsed(), result);
            if let expressions::Expr::LitExpr(lit) = result {
                match lit {
                    CPLiteral::FlatLiteral(CPFlatLiteral::OnboardingResult(r)) => {
                        Ok(r)
                    }, 
                    _ => Err(expressions::Error::new("literal has wrong type"))
                }
            } else {
                Err(expressions::Error::new("did not evaluate to a literal"))
            }
        }
        .boxed()
    }
}

impl Default for OnboardingPolicy {
    fn default() -> Self {
        let raw_pol = "
            fn onboarding_policy(od: OnboardingData) -> OnboardingResult {
                OnboardingResult::ErrStr(\"Onboarding disabled by default, update the onboarding policy first.\")
            }
        ";
        let policy = policies::OnboardingPolicy::from_buf(raw_pol).unwrap();
        let env = CPEnv::new(policy.program());
        OnboardingPolicy { policy, env }
    }
}