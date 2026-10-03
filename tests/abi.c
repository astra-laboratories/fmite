/* Header conformance: every function of the FMI 3.0.1 headers, assigned to the
 * header's own function type. Linked against an fmite FMU, this fails to link if a
 * symbol is missing and fails to compile if the headers disagree with themselves;
 * running it calls fmi3GetVersion through the header's prototype.
 *
 * The headers are the standard's, vendored under BSD-2-Clause: see headers/LICENSE.txt.
 */

#include <stdio.h>
#include <string.h>

#include "fmi3Functions.h"

int main(void) {
    fmi3GetVersionTYPE *getVersion = fmi3GetVersion;
    fmi3SetDebugLoggingTYPE *setDebugLogging = fmi3SetDebugLogging;
    fmi3InstantiateModelExchangeTYPE *instantiateModelExchange = fmi3InstantiateModelExchange;
    fmi3InstantiateCoSimulationTYPE *instantiateCoSimulation = fmi3InstantiateCoSimulation;
    fmi3InstantiateScheduledExecutionTYPE *instantiateScheduledExecution = fmi3InstantiateScheduledExecution;
    fmi3FreeInstanceTYPE *freeInstance = fmi3FreeInstance;
    fmi3EnterInitializationModeTYPE *enterInitializationMode = fmi3EnterInitializationMode;
    fmi3ExitInitializationModeTYPE *exitInitializationMode = fmi3ExitInitializationMode;
    fmi3EnterEventModeTYPE *enterEventMode = fmi3EnterEventMode;
    fmi3TerminateTYPE *terminate = fmi3Terminate;
    fmi3ResetTYPE *reset = fmi3Reset;
    fmi3GetFloat32TYPE *getFloat32 = fmi3GetFloat32;
    fmi3GetFloat64TYPE *getFloat64 = fmi3GetFloat64;
    fmi3GetInt8TYPE *getInt8 = fmi3GetInt8;
    fmi3GetUInt8TYPE *getUInt8 = fmi3GetUInt8;
    fmi3GetInt16TYPE *getInt16 = fmi3GetInt16;
    fmi3GetUInt16TYPE *getUInt16 = fmi3GetUInt16;
    fmi3GetInt32TYPE *getInt32 = fmi3GetInt32;
    fmi3GetUInt32TYPE *getUInt32 = fmi3GetUInt32;
    fmi3GetInt64TYPE *getInt64 = fmi3GetInt64;
    fmi3GetUInt64TYPE *getUInt64 = fmi3GetUInt64;
    fmi3GetBooleanTYPE *getBoolean = fmi3GetBoolean;
    fmi3GetStringTYPE *getString = fmi3GetString;
    fmi3GetBinaryTYPE *getBinary = fmi3GetBinary;
    fmi3GetClockTYPE *getClock = fmi3GetClock;
    fmi3SetFloat32TYPE *setFloat32 = fmi3SetFloat32;
    fmi3SetFloat64TYPE *setFloat64 = fmi3SetFloat64;
    fmi3SetInt8TYPE *setInt8 = fmi3SetInt8;
    fmi3SetUInt8TYPE *setUInt8 = fmi3SetUInt8;
    fmi3SetInt16TYPE *setInt16 = fmi3SetInt16;
    fmi3SetUInt16TYPE *setUInt16 = fmi3SetUInt16;
    fmi3SetInt32TYPE *setInt32 = fmi3SetInt32;
    fmi3SetUInt32TYPE *setUInt32 = fmi3SetUInt32;
    fmi3SetInt64TYPE *setInt64 = fmi3SetInt64;
    fmi3SetUInt64TYPE *setUInt64 = fmi3SetUInt64;
    fmi3SetBooleanTYPE *setBoolean = fmi3SetBoolean;
    fmi3SetStringTYPE *setString = fmi3SetString;
    fmi3SetBinaryTYPE *setBinary = fmi3SetBinary;
    fmi3SetClockTYPE *setClock = fmi3SetClock;
    fmi3GetNumberOfVariableDependenciesTYPE *getNumberOfVariableDependencies = fmi3GetNumberOfVariableDependencies;
    fmi3GetVariableDependenciesTYPE *getVariableDependencies = fmi3GetVariableDependencies;
    fmi3GetFMUStateTYPE *getFMUState = fmi3GetFMUState;
    fmi3SetFMUStateTYPE *setFMUState = fmi3SetFMUState;
    fmi3FreeFMUStateTYPE *freeFMUState = fmi3FreeFMUState;
    fmi3SerializedFMUStateSizeTYPE *serializedFMUStateSize = fmi3SerializedFMUStateSize;
    fmi3SerializeFMUStateTYPE *serializeFMUState = fmi3SerializeFMUState;
    fmi3DeserializeFMUStateTYPE *deserializeFMUState = fmi3DeserializeFMUState;
    fmi3GetDirectionalDerivativeTYPE *getDirectionalDerivative = fmi3GetDirectionalDerivative;
    fmi3GetAdjointDerivativeTYPE *getAdjointDerivative = fmi3GetAdjointDerivative;
    fmi3EnterConfigurationModeTYPE *enterConfigurationMode = fmi3EnterConfigurationMode;
    fmi3ExitConfigurationModeTYPE *exitConfigurationMode = fmi3ExitConfigurationMode;
    fmi3GetIntervalDecimalTYPE *getIntervalDecimal = fmi3GetIntervalDecimal;
    fmi3GetIntervalFractionTYPE *getIntervalFraction = fmi3GetIntervalFraction;
    fmi3GetShiftDecimalTYPE *getShiftDecimal = fmi3GetShiftDecimal;
    fmi3GetShiftFractionTYPE *getShiftFraction = fmi3GetShiftFraction;
    fmi3SetIntervalDecimalTYPE *setIntervalDecimal = fmi3SetIntervalDecimal;
    fmi3SetIntervalFractionTYPE *setIntervalFraction = fmi3SetIntervalFraction;
    fmi3SetShiftDecimalTYPE *setShiftDecimal = fmi3SetShiftDecimal;
    fmi3SetShiftFractionTYPE *setShiftFraction = fmi3SetShiftFraction;
    fmi3EvaluateDiscreteStatesTYPE *evaluateDiscreteStates = fmi3EvaluateDiscreteStates;
    fmi3UpdateDiscreteStatesTYPE *updateDiscreteStates = fmi3UpdateDiscreteStates;
    fmi3EnterContinuousTimeModeTYPE *enterContinuousTimeMode = fmi3EnterContinuousTimeMode;
    fmi3CompletedIntegratorStepTYPE *completedIntegratorStep = fmi3CompletedIntegratorStep;
    fmi3SetTimeTYPE *setTime = fmi3SetTime;
    fmi3SetContinuousStatesTYPE *setContinuousStates = fmi3SetContinuousStates;
    fmi3GetContinuousStateDerivativesTYPE *getContinuousStateDerivatives = fmi3GetContinuousStateDerivatives;
    fmi3GetEventIndicatorsTYPE *getEventIndicators = fmi3GetEventIndicators;
    fmi3GetContinuousStatesTYPE *getContinuousStates = fmi3GetContinuousStates;
    fmi3GetNominalsOfContinuousStatesTYPE *getNominalsOfContinuousStates = fmi3GetNominalsOfContinuousStates;
    fmi3GetNumberOfEventIndicatorsTYPE *getNumberOfEventIndicators = fmi3GetNumberOfEventIndicators;
    fmi3GetNumberOfContinuousStatesTYPE *getNumberOfContinuousStates = fmi3GetNumberOfContinuousStates;
    fmi3EnterStepModeTYPE *enterStepMode = fmi3EnterStepMode;
    fmi3GetOutputDerivativesTYPE *getOutputDerivatives = fmi3GetOutputDerivatives;
    fmi3DoStepTYPE *doStep = fmi3DoStep;
    fmi3ActivateModelPartitionTYPE *activateModelPartition = fmi3ActivateModelPartition;

    const void *all[] = {
        (const void *)getVersion,
        (const void *)setDebugLogging,
        (const void *)instantiateModelExchange,
        (const void *)instantiateCoSimulation,
        (const void *)instantiateScheduledExecution,
        (const void *)freeInstance,
        (const void *)enterInitializationMode,
        (const void *)exitInitializationMode,
        (const void *)enterEventMode,
        (const void *)terminate,
        (const void *)reset,
        (const void *)getFloat32,
        (const void *)getFloat64,
        (const void *)getInt8,
        (const void *)getUInt8,
        (const void *)getInt16,
        (const void *)getUInt16,
        (const void *)getInt32,
        (const void *)getUInt32,
        (const void *)getInt64,
        (const void *)getUInt64,
        (const void *)getBoolean,
        (const void *)getString,
        (const void *)getBinary,
        (const void *)getClock,
        (const void *)setFloat32,
        (const void *)setFloat64,
        (const void *)setInt8,
        (const void *)setUInt8,
        (const void *)setInt16,
        (const void *)setUInt16,
        (const void *)setInt32,
        (const void *)setUInt32,
        (const void *)setInt64,
        (const void *)setUInt64,
        (const void *)setBoolean,
        (const void *)setString,
        (const void *)setBinary,
        (const void *)setClock,
        (const void *)getNumberOfVariableDependencies,
        (const void *)getVariableDependencies,
        (const void *)getFMUState,
        (const void *)setFMUState,
        (const void *)freeFMUState,
        (const void *)serializedFMUStateSize,
        (const void *)serializeFMUState,
        (const void *)deserializeFMUState,
        (const void *)getDirectionalDerivative,
        (const void *)getAdjointDerivative,
        (const void *)enterConfigurationMode,
        (const void *)exitConfigurationMode,
        (const void *)getIntervalDecimal,
        (const void *)getIntervalFraction,
        (const void *)getShiftDecimal,
        (const void *)getShiftFraction,
        (const void *)setIntervalDecimal,
        (const void *)setIntervalFraction,
        (const void *)setShiftDecimal,
        (const void *)setShiftFraction,
        (const void *)evaluateDiscreteStates,
        (const void *)updateDiscreteStates,
        (const void *)enterContinuousTimeMode,
        (const void *)completedIntegratorStep,
        (const void *)setTime,
        (const void *)setContinuousStates,
        (const void *)getContinuousStateDerivatives,
        (const void *)getEventIndicators,
        (const void *)getContinuousStates,
        (const void *)getNominalsOfContinuousStates,
        (const void *)getNumberOfEventIndicators,
        (const void *)getNumberOfContinuousStates,
        (const void *)enterStepMode,
        (const void *)getOutputDerivatives,
        (const void *)doStep,
        (const void *)activateModelPartition,
    };
    for (size_t i = 0; i < sizeof all / sizeof all[0]; i++) {
        if (all[i] == NULL) {
            return 1;
        }
    }
    printf("%zu %s\n", sizeof all / sizeof all[0], getVersion());
    return strcmp(getVersion(), fmi3Version) != 0;
}
